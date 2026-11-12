//! A scope check for emitted JavaScript: names the tracer generated that the
//! code reads where no declaration is in scope. `node --check` accepts such
//! code; running it throws a `ReferenceError`.

use oxc_allocator::Allocator;
use oxc_ast::AstKind;
use oxc_semantic::{AstNodes, NodeId};
use oxc_span::GetSpan;

use crate::analysis;

/// The prefixes of names the tracer and emitter generate. Any other free
/// name is a global the code expects (`host`, `vmDecodeString`, …).
const GENERATED: &[&str] = &["$", "trace_value_", "vm_sub_", "vm_loop_"];

/// The generated names `code` reads without a declaration in scope, or
/// before their `let` runs (in the same function), sorted and without
/// repeats; an error when `code` does not parse.
pub fn undeclared_generated_names(code: &str) -> Result<Vec<String>, String> {
    let allocator = Allocator::default();
    let parsed = analysis::parse(&allocator, code);
    if let Some(error) = parsed.errors.first() {
        return Err(format!("emitted JavaScript does not parse: {error}"));
    }
    let (scoping, nodes) = analysis::analyze(&parsed.program);
    let (scoping, nodes) = (scoping, nodes);
    let generated = |name: &str| GENERATED.iter().any(|prefix| name.starts_with(prefix));
    let mut names: Vec<String> = scoping
        .root_unresolved_references()
        .iter()
        .map(|(name, _)| name.to_string())
        .filter(|name| generated(name))
        .collect();
    // A read before the `let` in the same function throws too.
    for symbol in scoping.symbol_ids() {
        let name = scoping.symbol_name(symbol);
        if !generated(name)
            || !scoping
                .symbol_flags(symbol)
                .contains(oxc_semantic::SymbolFlags::BlockScopedVariable)
        {
            continue;
        }
        let declaration = scoping.symbol_declaration(symbol);
        let declared_at = scoping.symbol_span(symbol).start;
        let function = enclosing_function(&nodes, declaration);
        for reference in scoping.get_resolved_reference_ids(symbol) {
            let node = scoping.get_reference(*reference).node_id();
            if nodes.get_node(node).span().start < declared_at
                && enclosing_function(&nodes, node) == function
            {
                names.push(name.to_string());
            }
        }
    }
    names.sort();
    names.dedup();
    Ok(names)
}

/// Every name `code` reads with no declaration anywhere in it: the globals
/// it expects, sorted; an error when `code` does not parse.
pub fn free_names(code: &str) -> Result<Vec<String>, String> {
    let allocator = Allocator::default();
    let parsed = analysis::parse(&allocator, code);
    if let Some(error) = parsed.errors.first() {
        return Err(format!("JavaScript does not parse: {error}"));
    }
    let (scoping, _) = analysis::analyze(&parsed.program);
    let mut names: Vec<String> = scoping
        .root_unresolved_references()
        .iter()
        .map(|(name, _)| name.to_string())
        .collect();
    names.sort();
    Ok(names)
}

/// The names `code` reads and does not declare, and every name it declares
/// in any scope.
pub fn free_and_bound_names(code: &str) -> Result<(Vec<String>, Vec<String>), String> {
    let allocator = Allocator::default();
    let parsed = analysis::parse(&allocator, code);
    if let Some(error) = parsed.errors.first() {
        return Err(format!("JavaScript does not parse: {error}"));
    }
    let (scoping, _) = analysis::analyze(&parsed.program);
    let free = scoping
        .root_unresolved_references()
        .iter()
        .map(|(name, _)| name.to_string())
        .collect();
    let bound = scoping.symbol_names().map(str::to_string).collect();
    Ok((free, bound))
}

/// The function `node` is in, if any.
fn enclosing_function(nodes: &AstNodes<'_>, node: NodeId) -> Option<NodeId> {
    nodes.ancestors(node).find_map(|ancestor| {
        matches!(
            ancestor.kind(),
            AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)
        )
        .then(|| ancestor.id())
    })
}

#[cfg(test)]
mod tests {
    use super::undeclared_generated_names;

    #[test]
    fn a_read_outside_the_block_of_its_let_is_found() {
        let code = "function f(x) { if (x) { let $SSA_1 = 1; } return $SSA_1; }";
        assert_eq!(undeclared_generated_names(code).unwrap(), ["$SSA_1"]);
    }

    #[test]
    fn a_read_before_its_let_is_found() {
        let code = "function f() { let $SSA_2 = $SSA_1; let $SSA_1 = 1; return $SSA_2; }";
        assert_eq!(undeclared_generated_names(code).unwrap(), ["$SSA_1"]);
    }

    #[test]
    fn hoisted_functions_globals_and_declared_names_are_fine() {
        let code = "function f() { let $SSA_1 = vm_sub_1(host); return $SSA_1; \
                    function vm_sub_1(a) { let $SSA_2 = a; return $SSA_2; } }";
        assert!(undeclared_generated_names(code).unwrap().is_empty());
    }
}
