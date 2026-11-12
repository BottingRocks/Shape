//! Read a `$KEY_` slot that only ever holds a global's value as that value.
//!
//! The program's setup stores built-ins and host objects in slots:
//!
//! ```js
//! $KEY_237 = Array.prototype.push;
//! $KEY_343 = window;
//! $KEY_5 = $KEY_343.document;
//! $KEY_288 = RegExp("[xX][nN]--");
//! ```
//!
//! Globals are taken not to change (the ECMAScript built-ins and `window`,
//! `self`, `document`, `navigator`), so each read of such a slot is the
//! global path itself, and the slot goes. A `RegExp` of literal source and
//! flags is written as a regex literal: a fresh one per read is the same
//! when the flags have no `g` or `y`, the only ones that keep state.
//!
//! The slot is written once, by its declaration or by a statement
//! `$KEY_n = v;` directly in the function that declares it, and no read
//! in that function comes before the store. A read inside a function the
//! thread creates is taken to run after the setup stored it, as the
//! original would otherwise have read `undefined` (and, for a native,
//! called it). The global's name must not be shadowed where it is read.
//! Inlining `window` makes `$KEY_343.document` a global path too, so the
//! pass repeats until nothing changes.

use std::collections::{BTreeMap, HashMap};

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast::AstKind;
use oxc_semantic::{AstNodes, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};

use super::super::primitives::is_stable_global;
use super::super::support::owner;
use crate::analysis;
use super::super::support::whole_line;

pub(crate) fn inline_constant_slots(source: &str) -> Result<(String, usize), String> {
    let mut code = source.to_string();
    let mut total = 0;
    for _ in 0..16 {
        let (next, count) = inline_once(&code)?;
        if count == 0 {
            break;
        }
        (code, total) = (next, total + count);
    }
    Ok((code, total))
}

fn stable_root(name: &str) -> bool {
    is_stable_global(name) || matches!(name, "window" | "self" | "document" | "navigator")
}

/// A global path's root name, or None: `Array.prototype.push` → `Array`.
fn global_path<'e>(scoping: &Scoping, expression: &'e Expression<'_>) -> Option<&'e str> {
    match expression.without_parentheses() {
        Expression::Identifier(name) => {
            (stable_root(&name.name) && scoping.get_reference(name.reference_id()).symbol_id().is_none()).then_some(name.name.as_str())
        }
        Expression::StaticMemberExpression(member) if !member.optional => global_path(scoping, &member.object),
        Expression::ComputedMemberExpression(member) if !member.optional && matches!(member.expression, Expression::StringLiteral(_)) => {
            global_path(scoping, &member.object)
        }
        _ => None,
    }
}

/// `RegExp("p")` / `RegExp("p", "f")` (or `new RegExp`) without `g`/`y`,
/// as a literal that parses to the same pattern.
pub(crate) fn regexp_literal(scoping: &Scoping, expression: &Expression<'_>) -> Option<String> {
    let (callee, arguments) = match expression.without_parentheses() {
        Expression::CallExpression(call) if !call.optional => (&call.callee, &call.arguments),
        Expression::NewExpression(new) => (&new.callee, &new.arguments),
        _ => return None,
    };
    if global_path(scoping, callee) != Some("RegExp") || !matches!(callee, Expression::Identifier(_)) {
        return None;
    }
    let (pattern, flags) = match arguments.as_slice() {
        [Argument::StringLiteral(pattern)] => (pattern.value.as_str(), ""),
        [Argument::StringLiteral(pattern), Argument::StringLiteral(flags)] => (pattern.value.as_str(), flags.value.as_str()),
        _ => return None,
    };
    if flags.contains(['g', 'y']) || !flags.chars().all(|flag| "dimsuv".contains(flag)) {
        return None;
    }
    let mut body = String::new();
    let mut chars = pattern.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                body.push('\\');
                body.push(chars.next()?);
            }
            '/' => body.push_str("\\/"),
            '\n' | '\r' | '\u{2028}' | '\u{2029}' => return None,
            c => body.push(c),
        }
    }
    if body.is_empty() {
        body.push_str("(?:)");
    }
    let literal = format!("/{body}/{flags}");
    // The literal must parse, pattern included.
    let allocator = Allocator::default();
    let text = format!("({literal});");
    let parsed = analysis::parse(&allocator, &text);
    parsed.errors.is_empty().then_some(literal)
}

struct Slot {
    value: String,
    /// The store statement, when the value is not the declaration's.
    store: Option<Span>,
    reads: Vec<(Span, oxc_semantic::NodeId)>,
}

fn inline_once(source: &str) -> Result<(String, usize), String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut slots: BTreeMap<SymbolId, Slot> = BTreeMap::new();
    for symbol in scoping.symbol_ids() {
        if let Some(slot) = candidate(source, &scoping, &nodes, symbol) {
            slots.insert(symbol, slot);
        }
    }
    // A slot whose value reads another slot waits for that one to go.
    let spans: Vec<Span> = slots.values().flat_map(|slot| slot.reads.iter().map(|(span, _)| *span)).collect();
    slots.retain(|symbol, slot| {
        let declaration = nodes.kind(scoping.symbol_declaration(*symbol)).span();
        let value_span = slot.store.unwrap_or(declaration);
        !spans.iter().any(|read| value_span.start <= read.start && read.end <= value_span.end)
    });
    if slots.is_empty() {
        return Ok((source.into(), 0));
    }
    let mut edits = Vec::new();
    // Declarations lose their inlined declarators together.
    let mut declarations: HashMap<Span, (Vec<String>, &str)> = HashMap::new();
    for symbol in slots.keys() {
        let declarator = scoping.symbol_declaration(*symbol);
        if let AstKind::VariableDeclaration(declaration) = nodes.parent_kind(declarator) {
            declarations.entry(declaration.span).or_insert_with(|| {
                let kept = declaration
                    .declarations
                    .iter()
                    .filter(|d| !d.id.get_binding_identifier().is_some_and(|id| slots.contains_key(&id.symbol_id())))
                    .map(|d| source[d.span.start as usize..d.span.end as usize].to_string())
                    .collect();
                (kept, declaration.kind.as_str())
            });
        }
    }
    for (span, (kept, kind)) in &declarations {
        if kept.is_empty() {
            edits.push(analysis::Edit::delete(whole_line(source, *span)));
        } else {
            edits.push(analysis::Edit::replace(*span, format!("{kind} {};", kept.join(", "))));
        }
    }
    for slot in slots.values() {
        if let Some(store) = slot.store {
            edits.push(analysis::Edit::delete(whole_line(source, store)));
        }
        for (read, _) in &slot.reads {
            edits.push(analysis::Edit::replace(*read, slot.value.clone()));
        }
    }
    let count = slots.len();
    let output = super::super::support::apply_checked(source, edits, "constant slot")?;
    Ok((output, count))
}

fn candidate(source: &str, scoping: &Scoping, nodes: &AstNodes<'_>, symbol: SymbolId) -> Option<Slot> {
    if !scoping.symbol_name(symbol).starts_with("$KEY_") {
        return None;
    }
    let declaration = scoping.symbol_declaration(symbol);
    let AstKind::VariableDeclarator(declarator) = nodes.kind(declaration) else {
        return None;
    };
    let AstKind::VariableDeclaration(_) = nodes.parent_kind(declaration) else {
        return None;
    };
    let home = owner(nodes, declaration);
    let mut writes = Vec::new();
    let mut reads = Vec::new();
    for reference in scoping.get_resolved_references(symbol) {
        let span = nodes.kind(reference.node_id()).span();
        if reference.is_write() {
            if reference.is_read() {
                return None;
            }
            writes.push(reference.node_id());
        } else {
            reads.push((span, reference.node_id()));
        }
    }
    // The value: the declaration's, or one store statement's.
    let (value, store): (&Expression<'_>, Option<Span>) = match (&declarator.init, writes.as_slice()) {
        (Some(init), []) => (init, None),
        (None, [write]) => {
            let AstKind::AssignmentExpression(assignment) = nodes.parent_kind(*write) else {
                return None;
            };
            if assignment.operator != AssignmentOperator::Assign {
                return None;
            }
            let assignment_node = nodes.parent_id(*write);
            let AstKind::ExpressionStatement(statement) = nodes.parent_kind(assignment_node) else {
                return None;
            };
            // Directly in the declaring function's body.
            let statement_node = nodes.parent_id(assignment_node);
            if !matches!(nodes.parent_kind(statement_node), AstKind::FunctionBody(_) | AstKind::Program(_))
                || owner(nodes, statement_node) != home
            {
                return None;
            }
            (&assignment.right, Some(statement.span))
        }
        _ => return None,
    };
    let (text, root) = if let Some(root) = global_path(scoping, value) {
        (source[value.span().start as usize..value.span().end as usize].to_string(), Some(root.to_string()))
    } else {
        (regexp_literal(scoping, value)?, None)
    };
    let after = store.map_or(declarator.span.end, |span| span.end);
    for (span, node) in &reads {
        // No read in the declaring function's own code before the value.
        if owner(nodes, *node) == home && span.start < after {
            return None;
        }
        // The global's name means the global where it is read.
        if let Some(root) = &root {
            if scoping.find_binding(nodes.get_node(*node).scope_id(), root.as_str().into()).is_some() {
                return None;
            }
        }
        // `$KEY_n(x)` calls with no receiver, `Object.keys(x)` with
        // `Object`: only a static method that ignores `this` is the same.
        if let AstKind::CallExpression(call) = nodes.parent_kind(*node) {
            if call.callee.span() == *span && !receiver_free(value) {
                return None;
            }
        }
    }
    Some(Slot { value: text, store, reads })
}

/// A name, or a method of a namespace whose methods ignore `this`
/// (`Object.keys`, `Math.max`, `JSON.parse`, `Reflect.apply`, …).
fn receiver_free(value: &Expression<'_>) -> bool {
    match value.without_parentheses() {
        Expression::Identifier(_) => true,
        member @ (Expression::StaticMemberExpression(_) | Expression::ComputedMemberExpression(_)) => {
            let member = member.as_member_expression().unwrap();
            matches!(member.object().without_parentheses(), Expression::Identifier(root)
                if matches!(root.name.as_str(), "Object" | "Math" | "JSON" | "Reflect" | "Number" | "String"))
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inlines_globals_and_regexps() {
        let source = "function $THREAD_1() {\n\tlet $KEY_1, $KEY_2, $KEY_3, $KEY_4, $KEY_6, $KEY_9;\n\t$KEY_6 = Object.defineProperty;\n\t$KEY_6(o, \"x\", {});\n\tfunction g(o) {\n\t\treturn [$KEY_1.call(o, 1), $KEY_3.title, $KEY_4.test(o)];\n\t}\n\t$KEY_1 = Array.prototype.push;\n\t$KEY_2 = window;\n\t$KEY_3 = $KEY_2.document;\n\t$KEY_4 = RegExp(\"^[xX][nN]--/\", \"i\");\n\treturn g;\n}\n";
        let (code, n) = inline_constant_slots(source).unwrap();
        assert_eq!(n, 5);
        assert_eq!(code, "function $THREAD_1() {\n\tlet $KEY_9;\n\tObject.defineProperty(o, \"x\", {});\n\tfunction g(o) {\n\t\treturn [Array.prototype.push.call(o, 1), window.document.title, /^[xX][nN]--\\//i.test(o)];\n\t}\n\treturn g;\n}\n");
    }

    #[test]
    fn keeps_slots_read_early_written_twice_or_shadowed() {
        // Read before the store; written twice; `window` shadowed where read;
        // a stateful regex; a member path called with the slot's receiver.
        let source = "function $THREAD_1() {\n\tlet $KEY_1, $KEY_2, $KEY_3, $KEY_4, $KEY_5;\n\tf($KEY_1);\n\t$KEY_1 = Object.keys;\n\t$KEY_2 = Math;\n\t$KEY_2 = JSON;\n\tfunction g(window) {\n\t\treturn [$KEY_3, $KEY_4, $KEY_5(1)];\n\t}\n\t$KEY_3 = window;\n\t$KEY_4 = RegExp(\"a\", \"g\");\n\t$KEY_5 = Object.prototype.toString;\n\treturn [g, $KEY_2];\n}\n";
        assert_eq!(inline_constant_slots(source).unwrap().1, 0);
    }
}
