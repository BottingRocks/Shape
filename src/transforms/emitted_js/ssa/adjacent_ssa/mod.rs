//! Inline a single-use generated temporary into its immediately following use.
//!
//! The first statement is `let $SSA_n = init;`; the second must evaluate the
//! temporary first (return/throw, if test, lexical slot write, single-variable
//! initializer, or immutable helper argument), and the temporary must have
//! exactly one read and no writes. The initializer is substituted at that read,
//! parenthesized, or as `(0, value)` for a direct call callee so bare-call
//! `this` is preserved. Later call arguments are accepted only when moving a
//! value across initialized local reads cannot invoke a getter or change which
//! values those reads observe. Thread mode tracks reaching stores of owned
//! locals regardless of their names; shared snapshots stay protected. It can
//! consume one stored version while retaining the cell for later versions.
//! Computed assignment keys
//! retain their coercion after RHS evaluation. `with`/`eval` disables the pass. The caller
//! repeats it to a bounded fixpoint.

use std::collections::{BTreeMap, BTreeSet};

use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstKind};
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::{operator::AssignmentOperator, scope::ScopeFlags};

use super::super::index::SpanIndex;
use crate::analysis;

mod local_reads;
mod scan;
mod visit;
mod prefixes;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod once_consumer_tests;
#[cfg(test)]
mod point_prefix_tests;
#[cfg(test)]
mod branch_version_tests;

pub(crate) use local_reads::LocalReads;
pub(crate) use prefixes::first_evaluated_identifier;
pub(crate) use prefixes::begins_with_read;
use visit::*;
use prefixes::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdjacentSsaResult {
    pub code: String,
    pub inlined_bindings: usize,
}

/// Inline `let $SSA_n = expression; return $SSA_n` and the equivalent
/// `throw` or simple assignment handoff. Symbol resolution must prove the
/// binding has exactly one reference, which is the adjacent use.
pub fn inline_adjacent_ssa(source: &str) -> Result<AdjacentSsaResult, String> {
    inline_with_policy(source, false, &BTreeSet::new())
}

pub(crate) fn inline_thread_adjacent_ssa(
    source: &str,
    protected: &BTreeSet<String>,
) -> Result<AdjacentSsaResult, String> {
    inline_with_policy(source, true, protected)
}

fn inline_with_policy(
    source: &str,
    native_calls: bool,
    protected: &BTreeSet<String>,
) -> Result<AdjacentSsaResult, String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut local_reads = LocalReads::default();
    if native_calls {
        local_reads.visit_program(&program);
        local_reads.build(&scoping, &nodes, protected);
    }
    let cells = native_calls.then(|| {
        super::super::cell_facts::collect(
            source,
            &program,
            &scoping,
            &nodes,
            &local_reads,
            protected,
            source.contains("arguments")
                && super::super::dense_arrays::ordinary_prototypes(&program, &scoping),
        )
    });
    let mut observed_names: BTreeMap<String, BTreeSet<SymbolId>> = BTreeMap::new();
    if native_calls {
        for symbol in scoping.symbol_ids() {
            if scoping.get_resolved_references(symbol).any(|r| r.is_read()) {
                observed_names
                    .entry(scoping.symbol_name(symbol).to_owned())
                    .or_default()
                    .insert(symbol);
            }
        }
    }
    let mut scan = Scan {
        liveness: super::super::cell_liveness::collect(&program, &scoping),
        observed_names,
        protected,
        native_calls: native_calls && super::super::method_calls::ordinary_call_lookups(&program),
        source,
        scoping: &scoping,
        nodes: &nodes,
        local_reads: &local_reads,
        cells: cells.as_ref(),
        dynamic_scope: false,
        edits: Vec::new(),
        inlined: 0,
    };
    scan.visit_program(&program);
    if scan.dynamic_scope || scan.edits.is_empty() {
        return Ok(AdjacentSsaResult {
            code: source.to_owned(),
            inlined_bindings: 0,
        });
    }
    let applied = analysis::apply_edits(source, scan.edits);
    if !applied.rejected.is_empty() || applied.applied != scan.inlined * 2 {
        return Err("adjacent SSA edits overlap".into());
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output)
        .map_err(|error| format!("adjacent SSA output: {error}"))?;
    Ok(AdjacentSsaResult {
        code: applied.output,
        inlined_bindings: scan.inlined,
    })
}

struct Scan<'s> {
    liveness: super::super::cell_liveness::Facts,
    observed_names: BTreeMap<String, BTreeSet<SymbolId>>,
    protected: &'s BTreeSet<String>,
    native_calls: bool,
    source: &'s str,
    scoping: &'s Scoping,
    nodes: &'s oxc_semantic::AstNodes<'s>,
    local_reads: &'s LocalReads,
    cells: Option<&'s super::super::cell_facts::Facts>,
    dynamic_scope: bool,
    edits: Vec<analysis::Edit>,
    inlined: usize,
}

// A new for-let environment can shadow names in an otherwise movable RHS.
// Internal bindings travel with the initializer; all other names must retain
// their original resolution at the insertion point, including captures.

// Type facts alone do not make reads stable: prefix_read separately checks
// initialization and the absence of writes/captured mutations across the move.

