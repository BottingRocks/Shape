//! Which function a node runs in.

use oxc_ast::AstKind;
use oxc_semantic::{AstNodes, NodeId, Scoping, SymbolId};

/// The function, arrow or program `node` runs in.
pub(crate) fn owner(nodes: &AstNodes<'_>, node: NodeId) -> Option<NodeId> {
    nodes
        .ancestor_ids(node)
        .find(|&ancestor| matches!(nodes.kind(ancestor), AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) | AstKind::Program(_)))
}

/// The function or arrow `node` runs in; `None` at the top level.
pub(crate) fn function_of(nodes: &AstNodes<'_>, node: NodeId) -> Option<NodeId> {
    nodes
        .ancestor_ids(node)
        .find(|&ancestor| matches!(nodes.kind(ancestor), AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)))
}

/// Every reference to `symbol` is in the function that declares it: no
/// closure sees it, so no call can reach it.
pub(crate) fn unclosed(scoping: &Scoping, nodes: &AstNodes<'_>, symbol: SymbolId) -> bool {
    let home = owner(nodes, scoping.symbol_declaration(symbol));
    scoping.get_resolved_references(symbol).all(|reference| owner(nodes, reference.node_id()) == home)
}
