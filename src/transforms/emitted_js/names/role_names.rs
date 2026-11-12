//! Rename what cleanup leaves of `$SSA_`/`$CARRY_` by the role it plays.
//!
//! After forwarding, every remaining generated temporary is a real binding:
//! the tracer's names say only that it is a temporary. This stage (13c) keeps
//! each name's numeric suffix and replaces its prefix with what the binding
//! does, so a later reader or pass can tell the cases apart:
//!
//! | prefix | binding |
//! | --- | --- |
//! | `$LOOP_` | written again inside a loop it is declared outside of |
//! | `$JOIN_` | written in branches: the join of several arms |
//! | `$VAR_` | otherwise reassigned |
//! | `$TUPLE_` | a slot of a destructured helper tuple |
//! | `$SNAP_` | a copy of a `$KEY_`/`$CARRY_` slot that must keep its value |
//! | `$ONCE_` | read once, kept because moving it would reorder effects |
//! | `$REUSED_` | read more than once |
//! | `$UNUSED_` | never read (its initializer still runs) |
//!
//! Renaming is by resolved symbol: the declaration and every reference move
//! together. A pass-wide check declines if any new name already occurs in the
//! source. `with`/`eval` disables it.

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast::AstKind;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, NodeId, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};

use crate::analysis;
use crate::transforms::emitted_js::support::is_loop;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RoleNamesResult {
    pub code: String,
    pub renamed: usize,
}

pub(crate) fn name_generated_roles(source: &str) -> Result<RoleNamesResult, String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut dynamic = Dynamic::default();
    dynamic.visit_program(&program);
    if dynamic.found {
        return Ok(RoleNamesResult {
            code: source.into(),
            renamed: 0,
        });
    }
    // Every `$`-name in the source, including strings and comments.
    let taken: std::collections::BTreeSet<&str> = source
        .match_indices('$')
        .map(|(start, _)| {
            let tail = &source[start..];
            let end = tail[1..]
                .bytes()
                .position(|b| !(b.is_ascii_alphanumeric() || b == b'_' || b == b'$'))
                .map_or(tail.len(), |at| at + 1);
            &tail[..end]
        })
        .collect();
    let mut edits = Vec::new();
    let mut renamed = 0;
    for symbol in scoping.symbol_ids() {
        let name = scoping.symbol_name(symbol);
        let Some(suffix) = name
            .strip_prefix("$SSA_")
            .or_else(|| name.strip_prefix("$CARRY_"))
        else {
            continue;
        };
        let new_name = format!("{}{suffix}", role_of(&scoping, &nodes, symbol).prefix());
        // A clash declines the whole pass: renaming only some bindings would
        // leave a mix of tracer and role names.
        if taken.contains(new_name.as_str()) {
            return Ok(RoleNamesResult {
                code: source.into(),
                renamed: 0,
            });
        }
        edits.push(analysis::Edit::replace(scoping.symbol_span(symbol), new_name.clone()));
        for reference in scoping.get_resolved_references(symbol) {
            let span = nodes.kind(reference.node_id()).span();
            edits.push(analysis::Edit::replace(span, new_name.clone()));
        }
        renamed += 1;
    }
    if edits.is_empty() {
        return Ok(RoleNamesResult {
            code: source.into(),
            renamed: 0,
        });
    }
    let applied = analysis::apply_edits(source, edits);
    if !applied.rejected.is_empty() {
        return Err("role name edits overlap".into());
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output)
        .map_err(|error| format!("role name output: {error}"))?;
    Ok(RoleNamesResult {
        code: applied.output,
        renamed,
    })
}

#[derive(Default)]
struct Dynamic {
    found: bool,
}

impl<'a> Visit<'a> for Dynamic {
    fn visit_with_statement(&mut self, statement: &WithStatement<'a>) {
        self.found = true;
        walk::walk_with_statement(self, statement);
    }

    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        if identifier.name == "eval" {
            self.found = true;
        }
    }
}

/// What a generated binding left after cleanup is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Role {
    /// Written again inside a loop it is declared outside of.
    Loop,
    /// Written in branches: the join of several arms.
    Join,
    /// Otherwise reassigned.
    Var,
    /// A slot of a destructured helper tuple.
    Tuple,
    /// A copy of a `$KEY_`/`$CARRY_` slot that must keep its value.
    Snap,
    /// Read once, kept because moving it would reorder effects.
    Once,
    /// Read more than once.
    Reused,
    /// Never read; its initializer still runs.
    Unused,
}

impl Role {
    /// The name prefix the role stage gives a binding with this role.
    pub fn prefix(self) -> &'static str {
        match self {
            Role::Loop => "$LOOP_",
            Role::Join => "$JOIN_",
            Role::Var => "$VAR_",
            Role::Tuple => "$TUPLE_",
            Role::Snap => "$SNAP_",
            Role::Once => "$ONCE_",
            Role::Reused => "$REUSED_",
            Role::Unused => "$UNUSED_",
        }
    }
}

/// Classify a binding by how it is written and read. Shared by the role
/// stage and the census tooling, so both report the same roles.
pub fn role_of(scoping: &Scoping, nodes: &AstNodes<'_>, symbol: SymbolId) -> Role {
    let declaration = scoping.symbol_declaration(symbol);
    let references: Vec<_> = scoping.get_resolved_references(symbol).collect();
    let writes: Vec<NodeId> = references
        .iter()
        .filter(|reference| reference.is_write())
        .map(|reference| reference.node_id())
        .collect();
    let reads = references.iter().filter(|reference| reference.is_read()).count();
    if !writes.is_empty() {
        if writes.iter().any(|write| enters(nodes, declaration, *write, is_loop)) {
            return Role::Loop;
        }
        if writes.iter().any(|write| enters(nodes, declaration, *write, is_branch)) {
            return Role::Join;
        }
        return Role::Var;
    }
    if matches!(nodes.parent_kind(declaration), AstKind::ArrayPattern(_))
        || matches!(nodes.kind(declaration), AstKind::VariableDeclarator(declarator)
            if matches!(declarator.id, BindingPattern::ArrayPattern(_)))
    {
        return Role::Tuple;
    }
    if let AstKind::VariableDeclarator(declarator) = nodes.kind(declaration) {
        if let Some(Expression::Identifier(source)) = &declarator.init {
            if source.name.starts_with("$KEY_") || source.name.starts_with("$CARRY_") {
                return Role::Snap;
            }
        }
    }
    match reads {
        0 => Role::Unused,
        1 => Role::Once,
        _ => Role::Reused,
    }
}

fn statement_span(nodes: &AstNodes<'_>, node: NodeId) -> Span {
    nodes
        .ancestor_ids(node)
        .find(|id| matches!(nodes.kind(*id), AstKind::VariableDeclaration(_)))
        .map_or_else(|| nodes.kind(node).span(), |id| nodes.kind(id).span())
}

/// `write` sits under a construct matching `kind` that does not also
/// contain the declaration.
fn enters(nodes: &AstNodes<'_>, declaration: NodeId, write: NodeId, kind: fn(AstKind<'_>) -> bool) -> bool {
    let declared = statement_span(nodes, declaration);
    nodes.ancestor_ids(write).any(|ancestor| {
        kind(nodes.kind(ancestor)) && {
            let span = nodes.kind(ancestor).span();
            !(span.start <= declared.start && declared.end <= span.end)
        }
    })
}

fn is_branch(kind: AstKind<'_>) -> bool {
    matches!(
        kind,
        AstKind::IfStatement(_)
            | AstKind::ConditionalExpression(_)
            | AstKind::LogicalExpression(_)
            | AstKind::SwitchStatement(_)
            | AstKind::TryStatement(_)
    )
}

#[cfg(test)]
mod tests {
    use super::name_generated_roles;

    #[test]
    fn names_each_binding_by_its_role() {
        let source = "function f(o, c) {\n  let $CARRY_1 = 0;\n  for (;;) {\n    $CARRY_1 = $CARRY_1 + 1;\n  }\n  let $CARRY_2;\n  if (c) {\n    $CARRY_2 = 1;\n  } else {\n    $CARRY_2 = 2;\n  }\n  let [$SSA_3, $SSA_4] = g();\n  let $SSA_5 = $KEY_1;\n  let $SSA_6 = o.p;\n  h();\n  let $SSA_7 = o.q;\n  return [$CARRY_2, $SSA_3, $SSA_4, $SSA_5, $SSA_6, $SSA_7, $SSA_7];\n}";
        let result = name_generated_roles(source).unwrap();
        assert_eq!(result.renamed, 7, "{}", result.code);
        for name in ["$LOOP_1", "$JOIN_2", "$TUPLE_3", "$TUPLE_4", "$SNAP_5", "$ONCE_6", "$REUSED_7"] {
            assert!(result.code.contains(name), "{name}: {}", result.code);
        }
        assert!(!result.code.contains("$SSA_") && !result.code.contains("$CARRY_"), "{}", result.code);
    }

    #[test]
    fn declines_when_a_role_name_is_taken() {
        let source = "function f() {\n  let $SSA_1 = g();\n  let $ONCE_1 = 2;\n  return [$SSA_1, $ONCE_1];\n}";
        let result = name_generated_roles(source).unwrap();
        assert_eq!(result.renamed, 0);
        assert_eq!(result.code, source);
    }
}
