//! Oxc cleanup of JavaScript emitted by [`crate::emit_javascript`].
//!
//! The standalone declaration pass removes only unread declarations with
//! pure, non-throwing initializers. Linked-program passes also simplify SSA,
//! carries, property writes, and repeated closed VM helpers. Each pass has its
//! own proof boundary; see [`README.md`](README.md) and the individual modules.
//! The standalone pass skips dynamic scope and unsupported declaration forms.
//!
//! ## Coordinates and provenance
//!
//! The output is produced by exact, non-overlapping source edits. A wholly
//! removed declaration is replaced by an empty statement (`;`) so adjacent
//! tokens cannot be joined across the deletion; a partially removed declaration
//! keeps its keyword, semicolon, and surviving declarators. [`RemovedRange`]
//! records original input byte offsets. Bytes after the first removal move by
//! the total deleted length, so old offsets cannot be used as output offsets.
//!
//! An [`EmittedStatementMap`] is index-based. A whole declaration in the
//! primary emitted function body becomes an empty statement, so later output
//! statement indexes stay stable even though the removed entry has no useful
//! provenance. The standalone report marks the original mapping
//! [`StatementMapStatus::Invalidated`]. [`cleanup_emitted_javascript`]
//! regenerates that map (and `node_map`) exactly and reports
//! [`StatementMapStatus::Regenerated`]. Entries that pointed at a removed
//! statement are dropped; surviving entries keep their parsed output indexes.
//!
//! The module also exports an independent property-write pass,
//! [`simplify_property_writes`], which rewrites the emitter's exact
//! `Object.defineProperty(receiver, key, {writable:true, configurable:true,
//! enumerable:true, value})` data-descriptor shape to `receiver[key] = value`
//! only when the native authority, receiver, key, descriptor, and evaluation
//! order are all proven. See the [`simplify_property_writes`] documentation and
//! `docs/transforms/emitted-property-writes.md`. Its edits are expression-local, so
//! [`CleanedEmittedPropertyWrites`] preserves the caller's statement/node
//! maps instead of regenerating them.

// The passes, by family. Each module is also imported here under its own
// name, so `super::name` paths (and the re-exports below) reach it.
mod branches;
mod carries;
mod expressions;
mod facts;
mod helpers;
mod literals;
mod loops;
mod names;
mod pipeline;
mod program;
mod ssa;
mod storage;
mod support;

use branches::{literal_branch_join, branch_value, branch_handoffs, return_joins, conditional_values, flatten_exits, empty_else, branch_tails, blocks, empty_statements};
use carries::{carry_join, carry_coalesce, carry_self_assign, carry_lifetimes, dead_carry, equivalent_carry, join_channels, stable_channels};
use expressions::{tidy_expressions, negated_comparisons, numeric_coercions, method_calls, optional_chains, nullish_syntax, null_flags, discarded_values, bit_accumulators, path_temporaries, constant_slots, prototype_methods, empty_starts, let_runs, trailing_returns};
use facts::{cell_facts, cell_liveness, branch_facts};
use helpers::{vm_sub_dedupe, vm_sub_parameters, vm_sub_simplify, inline_vm_sub, expand_vm_sub, inline_small_helpers, inline_tuple_helpers, runtime_helpers, tuple_scalarize, tuple_unpack};
use literals::{property_writes, dense_arrays, array_initializers, array_format, array_fills, fold_literal_fills, move_store_declarations, nest_literals, point_literals, adjacent_literals, inline_single_use_literals};
use loops::{structured_loops, loop_counters, loop_counter_copies, copied_counters, loop_labels, loop_tests, index_counters, native_for_in};
use names::{role_names, short_generated_names, positional_names, name_loop_and_catch_bindings};
use pipeline::{stages, thread};
use program::{classes, arrows, thread_parameters, vm_values, vm_fingerprint, unwrap_host_globals, dotted_properties};
use ssa::{adjacent_ssa, alias_ssa, literal_ssa, snapshot_ssa, snapshot_copies, slot_forward, argument_snapshots, forward_values, merge_derived, local_values, dead_ssa, constant_values};
use storage::{unread_declarations, last_use_storage, fresh_values, private_array_reads, declaration_lifetimes, dead_locals, dead_key, local_vars, unused_keys, key_declarations, key_webs, redundant_initializers};
use support::{purity, primitives, index};

pub use array_format::compact_arrays;
pub(crate) use short_generated_names::shorten_generated_names;
pub use stages::{clean_function, finish_program};
pub use thread::cleanup_emitted_thread;
pub use thread::{
    cleanup_thread, cleanup_thread_with_options, ThreadCleanup, ThreadCleanupOptions,
};
pub(crate) use vm_sub_dedupe::share_vm_sub_helpers;
pub(crate) use vm_sub_parameters::name_vm_sub_parameters;
pub(crate) use vm_sub_simplify::simplify_vm_sub_bodies;

pub use adjacent_ssa::{inline_adjacent_ssa, AdjacentSsaResult};
pub(crate) use alias_ssa::inline_alias_ssa;
pub(crate) use branch_value::{fold_branch_values, fold_multi_branch_values, fold_plain_branch_assignments};
pub(crate) use carry_coalesce::coalesce_carry_copies;
pub(crate) use carry_join::inline_single_use_carry_joins;
pub(crate) use carry_self_assign::remove_carry_self_assignments;
pub(crate) use dead_carry::remove_dead_carries;
pub(crate) use flatten_exits::flatten_exits;
pub(crate) use forward_values::{forward_values, forward_values_and_property_reads, forward_values_once};
pub(crate) use inline_small_helpers::{
    hoist_helper_calls, hoist_remaining_helper_calls, inline_remaining_helpers, inline_small_helpers,
};
pub(crate) use inline_tuple_helpers::inline_tuple_helpers;
pub(crate) use arrows::arrow_callbacks;
pub(crate) use dotted_properties::dotted_properties;
pub(crate) use thread_parameters::thread_parameters;
pub(crate) use trailing_returns::drop_trailing_returns;
pub(crate) use unused_keys::remove_unused_keys;
pub(crate) use vm_values::replace_vm_values;
pub(crate) use branch_tails::hoist_branch_tails;
pub(crate) use classes::rebuild_classes;
pub(crate) use optional_chains::optional_chains;
pub(crate) use path_temporaries::inline_path_temporaries;
pub(crate) use prototype_methods::name_prototype_methods;
pub(crate) use positional_names::name_bindings_by_position;
pub(crate) use role_names::name_generated_roles;
pub use role_names::{role_of, Role};
pub(crate) use tidy_expressions::{tidy_final_expressions, resugar_expressions, tidy_expressions};
pub(crate) use dead_key::remove_unread_key_stores;
pub(crate) use dead_ssa::{
    remove_dead_ssa, remove_discarded_argument_reads, remove_noop_local_reads,
};
pub(crate) use empty_else::remove_empty_else_blocks;
pub(crate) use key_declarations::place_key_declarations;
pub(crate) use key_webs::split_block_key_webs;
pub(crate) use loop_counter_copies::merge_loop_counter_copies;
pub(crate) use loop_labels::remove_needless_loop_labels;
pub(crate) use native_for_in::native_for_in_loops;
pub(crate) use equivalent_carry::merge_equivalent_carries;
pub use fold_literal_fills::{fold_literal_fills, LiteralFillResult};
pub use inline_single_use_literals::{inline_single_use_literals, SingleUseLiteralResult};
pub(crate) use inline_vm_sub::inline_vm_sub_wrappers;
pub use inline_vm_sub::{inline_remaining_vm_subs, InlinedVmSubs};
pub use literal_branch_join::{collapse_literal_branch_joins, LiteralBranchJoinResult};
pub use literal_ssa::{inline_literal_ssa, LiteralSsaResult};
pub(crate) use loop_counters::lift_loop_counters;
pub(crate) use loop_tests::split_loop_tests;
pub(crate) use local_vars::clean_local_vars;
pub(crate) use adjacent_literals::inline_adjacent_literals;
pub(crate) use array_fills::fold_array_fills;
pub(crate) use move_store_declarations::move_store_declarations;
pub(crate) use let_runs::join_let_runs;
pub(crate) use bit_accumulators::fold_bit_accumulators;
pub(crate) use constant_slots::inline_constant_slots;
pub(crate) use copied_counters::step_copied_counters;
pub(crate) use empty_starts::tidy_empty_starts;
pub(crate) use negated_comparisons::flip_negated_comparisons;
pub(crate) use nest_literals::nest_literals;
pub(crate) use merge_derived::{merge_derived_values, split_single_reads};
pub(crate) use structured_loops::{reconstruct_loops, unwrap_single_pass_loops};
pub use name_loop_and_catch_bindings::{name_loop_and_catch_bindings, LoopCatchNamesResult};
pub use redundant_initializers::{remove_redundant_initializers, RedundantInitializerResult};
pub(crate) use tuple_scalarize::scalarize_literal_tuples;
pub(crate) use tuple_unpack::unpack_fresh_helper_tuples;
pub use unwrap_host_globals::{unwrap_host_globals, HostGlobalsResult};

pub use array_initializers::{
    simplify_array_constructions, simplify_cleaned_emitted_property_writes,
    ArrayConstructionCleanupError, ArrayConstructionCleanupOutcome, ArrayConstructionSkip,
    ArrayConstructionSkipReason, CleanedEmittedArrayConstructions,
};
pub use property_writes::{
    simplify_cleaned_emitted_javascript, simplify_emitted_property_writes,
    simplify_program_property_writes, simplify_property_writes,
    simplify_readable_program_property_writes, CleanedEmittedPropertyWrites,
    PropertyWriteCleanupError, PropertyWriteCleanupOutcome, PropertyWriteSkip,
    PropertyWriteSkipReason, MAX_PRESEED_ARRAY_ELEMENTS, MAX_PRESEED_OBJECT_KEYS,
};

pub use unread_declarations::{
    EmittedCleanupOptions, StatementMapStatus, RemovedRange, EmittedCleanupReport, EmittedCleanupResult, CleanedEmittedJavaScript, EmittedCleanupError, cleanup, cleanup_with_options, cleanup_emitted_javascript, cleanup_emitted_javascript_with_options,
};
