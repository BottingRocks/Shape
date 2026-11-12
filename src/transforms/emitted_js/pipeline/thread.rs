//! Function-local cleanup, callable independently of tracing and linking.

use oxc_allocator::Allocator;
use oxc_ast::ast::Statement;
use std::collections::BTreeSet;

use super::super::{
    coalesce_carry_copies, collapse_literal_branch_joins, expand_vm_sub::expand_vm_subs,
    inline_literal_ssa, inline_single_use_carry_joins, merge_equivalent_carries,
    remove_carry_self_assignments, remove_dead_carries, remove_dead_ssa, scalarize_literal_tuples,
    snapshot_ssa::inline_snapshot_ssa,
};
use crate::analysis;

#[derive(Debug)]
pub struct ThreadCleanup {
    pub rewritten_property_writes: usize,
    pub folded_arrays: usize,
    pub simplified_method_calls: usize,
    pub reused_slot_values: usize,
    pub reconstructed_loops: usize,
    pub flattened_blocks: usize,
    pub code: String,
    pub inlined_snapshots: usize,
    pub inlined_bindings: usize,
    pub rounds: usize,
    pub inlined_helper_calls: usize,
    pub removed_helpers: usize,
    pub remaining_helpers: usize,
}

/// Slots inherited from a creator or shared with not-yet-linked children.
/// Their reads/writes cannot be inferred from this thread's AST alone.
#[derive(Debug, Default)]
pub struct ThreadCleanupOptions {
    pub shared_slots: BTreeSet<u32>,
}

/// Clean one emitted thread (one function declaration, optionally containing
/// local helpers). No thread linking, global naming, or helper sharing runs.
/// Source offsets/maps from the original emission are invalidated.
pub fn cleanup_thread(source: &str) -> Result<ThreadCleanup, String> {
    // An empty child marker has no AST references yet. Without tracer facts,
    // conservatively protect every numeric slot when such a marker is present.
    use oxc_ast::ast::{BindingIdentifier, Function};
    use oxc_ast_visit::{walk, Visit};
    use oxc_syntax::scope::ScopeFlags;
    #[derive(Default)]
    struct Shared {
        marker: bool,
        slots: BTreeSet<u32>,
    }
    impl<'a> Visit<'a> for Shared {
        fn visit_function(&mut self, f: &Function<'a>, flags: ScopeFlags) {
            self.marker |=
                f.id.as_ref()
                    .is_some_and(|id| id.name.starts_with("$THREAD_"))
                    && f.body.as_ref().is_some_and(|body| {
                        body.statements.is_empty() && body.directives.is_empty()
                    });
            walk::walk_function(self, f, flags);
        }
        fn visit_binding_identifier(&mut self, id: &BindingIdentifier<'a>) {
            if let Some(slot) = id.name.strip_prefix("$KEY_").and_then(|s| s.parse().ok()) {
                self.slots.insert(slot);
            }
        }
    }
    let allocator = Allocator::default();
    let parsed = analysis::parse_program(&allocator, source)?;
    let mut shared = Shared::default();
    shared.visit_program(&parsed);
    let options = ThreadCleanupOptions {
        shared_slots: if shared.marker {
            shared.slots
        } else {
            BTreeSet::new()
        },
    };
    cleanup_thread_with_options(source, &options)
}

pub fn cleanup_thread_with_options(
    source: &str,
    options: &ThreadCleanupOptions,
) -> Result<ThreadCleanup, String> {
    cleanup_thread_impl(source, options, false)
}

/// Only the program tracer, after validating ProgramFacts, may provide this
/// stronger capture contract. Child free generated names must be KEY cells;
/// other generated temporaries cannot be captured by pending children.
pub fn cleanup_emitted_thread(
    source: &str,
    options: &ThreadCleanupOptions,
) -> Result<ThreadCleanup, String> {
    cleanup_thread_impl(source, options, true)
}

fn cleanup_thread_impl(
    source: &str,
    options: &ThreadCleanupOptions,
    checked_captures: bool,
) -> Result<ThreadCleanup, String> {
    let protected: BTreeSet<String> = options
        .shared_slots
        .iter()
        .map(|slot| format!("$KEY_{slot}"))
        .collect();
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    if !matches!(program.body.as_slice(), [Statement::FunctionDeclaration(_)]) {
        return Err("thread cleanup expects exactly one function declaration".into());
    }
    let expanded = expand_vm_subs(source)?;
    let (code, runtime_calls, runtime_removed) =
        super::super::runtime_helpers::inline_runtime_helpers(&expanded.code)?;
    let mut result = ThreadCleanup {
        reconstructed_loops: 0,
        flattened_blocks: 0,
        rewritten_property_writes: 0,
        folded_arrays: 0,
        simplified_method_calls: 0,
        reused_slot_values: 0,
        code,
        inlined_snapshots: 0,
        inlined_bindings: 0,
        rounds: 0,
        inlined_helper_calls: expanded.calls + runtime_calls,
        removed_helpers: expanded.removed + runtime_removed,
        remaining_helpers: expanded.remaining,
    };
    expression_sweep(&mut result, &protected, checked_captures)?;
    storage_sweep(&mut result, &protected, checked_captures)?;
    late_propagation(&mut result, &protected, checked_captures)?;
    branch_cells(&mut result, &protected, checked_captures)?;
    literal_relocation(&mut result, &protected, checked_captures)?;
    recycling(&mut result, &protected, checked_captures)?;
    reprint(&mut result)?;
    Ok(result)
}

/// The main sweep, to a fixed point: null flags, branch facts, channels, literals, carries, snapshots and SSA; then loops.
fn expression_sweep(result: &mut ThreadCleanup, protected: &BTreeSet<String>, checked_captures: bool) -> Result<(), String> {
    for _ in 0..24 {
        let (code, null_flags) =
            super::super::null_flags::eliminate_null_flags(&result.code, &protected, checked_captures)?;
        result.inlined_bindings += null_flags;
        let (code, branch_facts) = super::super::branch_facts::simplify_branch_facts(&code)?;
        let (code, channels) = super::super::join_channels::normalize_join_channels(&code, &protected)?;
        let (code, point_literals) =
            super::super::point_literals::inline_property_keys(&code, &protected, checked_captures)?;
        let (code, constants) = super::super::constant_values::fold_constant_values(&code)?;
        let (code, dense_arrays) =
            super::super::dense_arrays::fold_dense_arrays(&code, &protected, checked_captures)?;
        result.folded_arrays += dense_arrays;
        let arrays = super::super::simplify_array_constructions(&code).map_err(|e| e.to_string())?;
        result.folded_arrays += arrays.compacted_arrays;
        let dead_carry = remove_dead_carries(&arrays.code)?;
        let coalesced = coalesce_carry_copies(&dead_carry.code)?;
        let equivalent = merge_equivalent_carries(&coalesced.code)?;
        let joined = inline_single_use_carry_joins(&equivalent.code)?;
        let literal_joined = collapse_literal_branch_joins(&joined.code)?;
        let (code, assignment_branches) =
            super::super::branch_value::fold_branch_assignments(&literal_joined.code)?;
        let folded = super::super::branch_value::fold_thread_branch_values(&code)?;
        let (code, self_assignments) = remove_carry_self_assignments(&folded.code)?;
        let (code, repeated_tests) = super::super::conditional_values::collapse_repeated_tests(&code)?;
        let tuples = scalarize_literal_tuples(&code)?;
        let literal = inline_literal_ssa(&tuples.code)?;
        let (code, snapshots) = inline_snapshot_ssa(&literal.code, &protected)?;
        let (code, copies) = super::super::snapshot_copies::merge_snapshot_copies(&code, &protected)?;
        let alias = super::super::alias_ssa::inline_alias_ssa_with_protected(&code, &protected)?;
        let adjacent = super::super::adjacent_ssa::inline_thread_adjacent_ssa(&alias.code, &protected)?;
        // Preserve descriptor provenance until helper tuple/alias relays settle.
        let normalized = tuples.removed == 0
            && literal.inlined_bindings == 0
            && snapshots == 0
            && alias.inlined_bindings == 0
            && adjacent.inlined_bindings == 0;
        let (code, objects, property_changes) = if normalized {
            let literals =
                super::super::property_writes::fold_descriptor_literals(&adjacent.code, &protected)?;
            result.folded_arrays += literals.arrays;
            let properties =
                super::super::property_writes::simplify_thread_property_writes(&literals.code, &protected)
                    .map_err(|e| e.to_string())?;
            (
                properties.code,
                literals.constructions,
                properties.changed + literals.writes,
            )
        } else {
            (adjacent.code.clone(), 0, 0)
        };
        result.rewritten_property_writes += property_changes;
        let (code, methods) = super::super::method_calls::simplify_method_calls(&code, &protected)?;
        result.simplified_method_calls += methods;
        let (code, reused) = super::super::local_values::simplify_local_values(&code, &protected)?;
        let (code, early_dead) = super::super::dead_locals::remove_dead_locals(&code, &protected)?;
        let (code, early_numeric) = super::super::numeric_coercions::simplify_numeric_coercions(&code)?;
        let (code, forwarded) = super::super::slot_forward::forward_ssa_slots(&code, &protected)?;
        let dead = remove_dead_ssa(&code)?;
        let (code, noop_reads) = super::super::dead_ssa::remove_noop_local_reads(&dead.code)?;
        let (code, discarded) = super::super::discarded_values::remove_discarded_values(&code)?;
        let (code, dead_locals) = super::super::dead_locals::remove_dead_locals(&code, &protected)?;
        let (code, numeric_coercions) =
            super::super::numeric_coercions::simplify_numeric_coercions(&code)?;
        result.reused_slot_values += reused;
        let inlined = literal.inlined_bindings
            + snapshots
            + alias.inlined_bindings
            + adjacent.inlined_bindings
            + forwarded
            + copies;
        result.code = code;
        result.inlined_snapshots += snapshots;
        result.inlined_bindings += inlined;
        result.rounds += 1;
        if inlined == 0
            && dead.removed == 0
            && tuples.removed == 0
            && self_assignments == 0
            && repeated_tests == 0
            && dead_carry.removed_carries == 0
            && coalesced.removed_carries == 0
            && equivalent.removed == 0
            && joined.removed_carries == 0
            && literal_joined.collapsed == 0
            && folded.replaced_carries == 0
            && property_changes == 0
            && arrays.compacted_arrays == 0
            && methods == 0
            && reused == 0
            && noop_reads == 0
            && discarded == 0
            && dead_locals == 0
            && objects == 0
            && numeric_coercions == 0
            && early_dead == 0
            && early_numeric == 0
            && constants == 0
            && point_literals == 0
            && dense_arrays == 0
            && channels == 0
            && assignment_branches == 0
            && branch_facts == 0
            && null_flags == 0
        {
            break;
        }
    }
    let (code, loops) = super::super::structured_loops::reconstruct_loops(&result.code, &protected)?;
    result.code = code;
    result.reconstructed_loops = loops;
    Ok(())
}

/// Storage, once SSA structure has served the expression proofs: tail joins, handoffs, private arrays; then counters, inputs and blocks.
fn storage_sweep(result: &mut ThreadCleanup, protected: &BTreeSet<String>, checked_captures: bool) -> Result<(), String> {
    // Preserve SSA structure until expression and descriptor reconstruction
    // finish. Reusing mutable storage earlier obstructs those proofs.
    for _ in 0..24 {
        let (code, branch_facts) = super::super::branch_facts::simplify_branch_facts(&result.code)?;
        let (code, returns) =
            super::super::return_joins::eliminate_thread_tail_joins(&code, &protected, checked_captures)?;
        // Consumer sinking can expose an assignment diamond for its existing
        // destination. Recover that immutable result before lifetime cleanup.
        let code = if returns != 0 {
            let (code, _) = super::super::branch_value::fold_branch_assignments(&code)?;
            super::super::join_channels::normalize_join_channels(&code, &protected)?.0
        } else {
            code
        };
        let (code, cells) = super::super::carry_lifetimes::coalesce_local_handoffs(&code, &protected)?;
        let (code, array_reads) = super::super::private_array_reads::inline_private_array_reads(
            &code,
            &protected,
            checked_captures,
        )?;
        let (code, fresh) =
            super::super::fresh_values::inline_fresh_values(&code, &protected, checked_captures)?;
        let adjacent = super::super::adjacent_ssa::inline_thread_adjacent_ssa(&code, &protected)?;
        let code = adjacent.code;
        let (code, dead) = super::super::dead_locals::remove_dead_locals(&code, &protected)?;
        result.code = code;
        result.inlined_bindings +=
            cells + returns + fresh + adjacent.inlined_bindings + array_reads;
        if cells == 0
            && dead == 0
            && returns == 0
            && fresh == 0
            && adjacent.inlined_bindings == 0
            && branch_facts == 0
            && array_reads == 0
        {
            break;
        }
    }
    // Recovered numeric induction updates must be normalized before the
    // ordered-argument proof can recognize their private Number counters.
    result.code = super::super::numeric_coercions::simplify_numeric_coercions(&result.code)?.0;
    let (code, blocks) = super::super::blocks::flatten_blocks(&result.code)?;
    let (code, inputs) = super::super::argument_snapshots::inline_argument_snapshots(&code, &protected)?;
    result.code = code;
    result.inlined_bindings += inputs;
    result.flattened_blocks = blocks;
    Ok(())
}

/// Chains the flattened continuations expose, under the same dependency and capture checks.
fn late_propagation(result: &mut ThreadCleanup, protected: &BTreeSet<String>, checked_captures: bool) -> Result<(), String> {
    // Preserve the storage proofs already made in the original scopes, then
    // consume chains the flattened continuations expose using the same
    // dependency/capture checks. Single use alone never authorizes reordering.
    for _ in 0..24 {
        let (prepared, tightened) =
            super::super::declaration_lifetimes::tighten(&result.code, &protected, checked_captures)?;
        let (code, argument_reads) = super::super::dead_ssa::remove_discarded_argument_reads(&prepared)?;
        let (code, empty_else) = super::super::empty_else::remove_empty_else_blocks(&code)?;
        let (mut code, null_flags) =
            super::super::null_flags::eliminate_null_flags(&code, &protected, checked_captures)?;
        result.inlined_bindings += null_flags;
        if null_flags != 0 || tightened != 0 {
            code = remove_dead_carries(&code)?.code;
            code = super::super::branch_value::fold_branch_assignments(&code)?.0;
            code = super::super::branch_value::fold_thread_branch_values(&code)?.code;
            code = super::super::join_channels::normalize_join_channels(&code, &protected)?.0;
        }
        let (code, channel_arguments, channel_aliases) =
            super::super::stable_channels::eliminate_stable_channels(&code, &protected, checked_captures)?;
        result.inlined_bindings += channel_arguments + channel_aliases;
        let (code, nullish) = super::super::nullish_syntax::recover_nullish_syntax(&code)?;
        let literal = inline_literal_ssa(&code)?;
        let (code, snapshots) = inline_snapshot_ssa(&literal.code, &protected)?;
        let alias = super::super::alias_ssa::inline_alias_ssa_with_protected(&code, &protected)?;
        let (code, reused) = super::super::local_values::simplify_local_values(&alias.code, &protected)?;
        let adjacent = super::super::adjacent_ssa::inline_thread_adjacent_ssa(&code, &protected)?;
        let (code, methods) =
            super::super::method_calls::simplify_method_calls(&adjacent.code, &protected)?;
        let (code, dead) = super::super::dead_locals::remove_dead_locals(&code, &protected)?;
        // Flattening and late propagation can expose a copy-out in the same
        // statement list. Rebuild semantic facts rather than relying on the
        // scopes or reference counts from the earlier storage phase.
        let (code, cells) = super::super::carry_lifetimes::coalesce_local_handoffs(&code, &protected)?;
        // Removing snapshots may also make an allocation single-use, or make
        // a private array's complete use graph visible to its cell proof.
        // These are the same identity, capture and version checks used above;
        // late scheduling does not grant permission to move arbitrary reads.
        let (code, fresh) =
            super::super::fresh_values::inline_fresh_values(&code, &protected, checked_captures)?;
        let (code, array_reads) = super::super::private_array_reads::inline_private_array_reads(
            &code,
            &protected,
            checked_captures,
        )?;
        let (code, tail_joins) =
            super::super::return_joins::eliminate_thread_tail_joins(&code, &protected, checked_captures)?;
        result.simplified_method_calls += methods;
        result.code = code;
        result.inlined_bindings += literal.inlined_bindings
            + snapshots
            + alias.inlined_bindings
            + adjacent.inlined_bindings
            + cells
            + fresh
            + array_reads
            + tail_joins;
        result.inlined_snapshots += snapshots;
        result.reused_slot_values += reused;
        if tightened == 0
            && literal.inlined_bindings == 0
            && snapshots == 0
            && channel_arguments == 0
            && channel_aliases == 0
            && alias.inlined_bindings == 0
            && adjacent.inlined_bindings == 0
            && reused == 0
            && dead == 0
            && methods == 0
            && nullish == 0
            && null_flags == 0
            && argument_reads == 0
            && empty_else == 0
            && cells == 0
            && fresh == 0
            && array_reads == 0
            && tail_joins == 0
        {
            break;
        }
    }
    result.code = super::super::numeric_coercions::simplify_numeric_coercions(&result.code)?.0;
    result.code = super::super::empty_else::remove_empty_else_blocks(&result.code)?.0;
    result.code = super::super::dead_ssa::remove_discarded_argument_reads(&result.code)?.0;
    result.code =
        super::super::index_counters::recover_index_counters(&result.code, &protected, checked_captures)?
            .0;
    Ok(())
}

/// Short-lived, definitely assigned branch cells, after snapshots have claimed their storage.
fn branch_cells(result: &mut ThreadCleanup, protected: &BTreeSet<String>, checked_captures: bool) -> Result<(), String> {
    // Finish short-lived, definitely assigned branch cells only after existing
    // snapshots have claimed their storage. Rebuild facts between proposals.
    for _ in 0..24 {
        let (code, recycled) = super::super::last_use_storage::recycle_last_inputs(
            &result.code,
            &protected,
            checked_captures,
        )?;
        let (code, branches) =
            super::super::branch_handoffs::coalesce_branch_handoffs(&code, &protected, checked_captures)?;
        let (code, finished) = super::super::carry_lifetimes::finish_private_channels(&code, &protected)?;
        result.code = code;
        result.inlined_bindings += finished;
        result.inlined_bindings += branches + recycled;
        if finished == 0 && branches == 0 && recycled == 0 {
            break;
        }
        result.code = remove_carry_self_assignments(&result.code)?.0;
        result.code = super::super::dead_locals::remove_dead_locals(&result.code, &protected)?.0;
    }
    Ok(())
}

/// Identity-unobserved literals, once slot and snapshot propagation settles.
fn literal_relocation(result: &mut ThreadCleanup, protected: &BTreeSet<String>, checked_captures: bool) -> Result<(), String> {
    // Relocate identity-unobserved literals only after slot and snapshot
    // propagation settles. Earlier reconstruction can hide producer provenance
    // and make the main sweep retain additional snapshots.
    for _ in 0..4 {
        let (code, moved) = super::super::declaration_lifetimes::tighten_allocations(
            &result.code,
            &protected,
            checked_captures,
        )?;
        let (code, folded) =
            super::super::dense_arrays::fold_dense_arrays(&code, &protected, checked_captures)?;
        result.folded_arrays += folded;
        let (code, fresh) =
            super::super::fresh_values::inline_fresh_values(&code, &protected, checked_captures)?;
        let adjacent = super::super::adjacent_ssa::inline_thread_adjacent_ssa(&code, &protected)?;
        let (code, dead) = super::super::dead_locals::remove_dead_locals(&adjacent.code, &protected)?;
        result.inlined_bindings += fresh + adjacent.inlined_bindings;
        result.code = code;
        if moved == 0 && folded == 0 && fresh == 0 && adjacent.inlined_bindings == 0 && dead == 0 {
            break;
        }
    }
    Ok(())
}

/// Unrelated ended live ranges, after producer reconstruction.
fn recycling(result: &mut ThreadCleanup, protected: &BTreeSet<String>, checked_captures: bool) -> Result<(), String> {
    // Recycle unrelated ended live ranges only after producer reconstruction.
    // Doing this earlier hides the array/descriptor construction provenance.
    for _ in 0..24 {
        let (code, recycled) = super::super::last_use_storage::recycle_ended_cells(
            &result.code,
            &protected,
            checked_captures,
        )?;
        let (code, points) =
            super::super::point_literals::inline_point_literals(&code, &protected, checked_captures)?;
        let (code, constants) = super::super::constant_values::fold_constant_values(&code)?;
        let (code, branches) = super::super::branch_facts::simplify_branch_facts(&code)?;
        let code = if branches != 0 {
            let (code, blocks) = super::super::blocks::flatten_blocks(&code)?;
            result.flattened_blocks += blocks;
            code
        } else {
            code
        };
        let (code, self_assignments) = remove_carry_self_assignments(&code)?;
        let (code, dead) = super::super::dead_locals::remove_dead_locals(&code, &protected)?;
        let (code, discarded) = super::super::discarded_values::remove_discarded_values(&code)?;
        result.code = code;
        result.inlined_bindings += recycled;
        if recycled == 0
            && points == 0
            && constants == 0
            && branches == 0
            && self_assignments == 0
            && dead == 0
            && discarded == 0
        {
            break;
        }
    }
    Ok(())
}

/// Empty statements and blocks go, and the AST is printed once.
fn reprint(result: &mut ThreadCleanup) -> Result<(), String> {
    // Span edits conservatively parenthesize substitutions. Reprint the AST
    // once after all rewrites: Oxc retains precedence/receiver parentheses and
    // drops wrappers such as `(call());` and `($KEY)[property]`.
    result.code = super::super::empty_statements::remove_empty_statements(&result.code)?.0;
    result.code = super::super::empty_else::remove_empty_else_blocks(&result.code)?.0;
    let print_allocator = Allocator::default();
    let printed = analysis::parse_program(&print_allocator, &result.code)?;
    result.code = super::super::compact_arrays(&analysis::codegen(&printed))?;
    Ok(())
}
