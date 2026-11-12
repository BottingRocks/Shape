//! Recycle a private cell once its old value dies, including when its final use
//! computes a new snapshot. Computations stay in position and execute once.
//! Captured/shared cells and simultaneously live unequal versions stay separate.
use super::super::adjacent_ssa::LocalReads;
use crate::analysis;
use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstKind};
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::scope::ScopeFlags;
use std::collections::{BTreeMap, BTreeSet};

mod scan;
mod successors;
mod declarations;
#[cfg(test)]
mod tests;

use successors::*;
use declarations::*;

pub(crate) fn recycle_last_inputs(
    source: &str,
    protected: &BTreeSet<String>,
    checked: bool,
) -> Result<(String, usize), String> {
    recycle(source, protected, checked, false)
}

pub(crate) fn recycle_ended_cells(
    source: &str,
    protected: &BTreeSet<String>,
    checked: bool,
) -> Result<(String, usize), String> {
    recycle(source, protected, checked, true)
}

fn recycle(
    source: &str,
    protected: &BTreeSet<String>,
    checked: bool,
    ended: bool,
) -> Result<(String, usize), String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    if nodes.iter().any(|n|matches!(n.kind(),AstKind::WithStatement(_))
        || matches!(n.kind(),AstKind::IdentifierReference(id)if id.name=="eval")
        || !checked&&matches!(n.kind(),AstKind::Function(f)if f.id.as_ref().is_some_and(|id|id.name.starts_with("$THREAD_"))&&f.body.as_ref().is_some_and(|b|b.statements.is_empty()&&b.directives.is_empty()))) {
        return Ok((source.into(),0));
    }
    let mut locals = LocalReads::default();
    locals.visit_program(&program);
    locals.build(&scoping, &nodes, protected);
    let mut declarations = Declarations::default();
    super::super::support::for_each_list(&program, |list| declarations.body(list));
    let mut scan = Scan {
        source,
        protected,
        scoping: &scoping,
        nodes: &nodes,
        locals: &locals,
        declarations: declarations.items,
        edits: vec![],
        used: BTreeSet::new(),
        count: 0,
        ended,
    };
    scan.visit_program(&program);
    let result = analysis::apply_edits(source, scan.edits);
    if !result.rejected.is_empty() {
        return Err("last input storage edits overlap".into());
    }
    Ok((result.output, scan.count))
}
struct Scan<'s, 'a> {
    source: &'s str,
    protected: &'s BTreeSet<String>,
    scoping: &'s Scoping,
    nodes: &'s AstNodes<'a>,
    locals: &'s LocalReads,
    declarations: BTreeMap<SymbolId, SourceDeclaration>,
    edits: Vec<analysis::Edit>,
    used: BTreeSet<SymbolId>,
    count: usize,
    ended: bool,
}
fn generated(name: &str) -> bool {
    name.starts_with("$SSA_") || name.starts_with("$CARRY_")
}

// Only actual statement-list declarations may lend storage. A for-header's
// update is textually before its body but executes afterward, so textual last
// use alone would be an invalid proof for those bindings.

