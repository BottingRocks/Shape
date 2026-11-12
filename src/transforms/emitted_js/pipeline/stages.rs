//! The stage order of `shape trace program`: each function's cleanup, then
//! the whole program's naming and tidying.

use std::collections::BTreeSet;

use super::super::{
    arrow_callbacks, clean_local_vars, coalesce_carry_copies, collapse_literal_branch_joins,
    dotted_properties, drop_trailing_returns, flatten_exits, flip_negated_comparisons,
    fold_array_fills, fold_bit_accumulators, fold_branch_values, fold_literal_fills,
    fold_multi_branch_values, fold_plain_branch_assignments, forward_values,
    forward_values_and_property_reads, forward_values_once, hoist_branch_tails, hoist_helper_calls,
    hoist_remaining_helper_calls, inline_adjacent_literals, inline_adjacent_ssa, inline_alias_ssa,
    inline_constant_slots, inline_literal_ssa, inline_path_temporaries, inline_remaining_helpers,
    inline_single_use_carry_joins, inline_single_use_literals, inline_small_helpers,
    inline_tuple_helpers, inline_vm_sub_wrappers, join_let_runs, lift_loop_counters,
    merge_derived_values, merge_loop_counter_copies, move_store_declarations, name_bindings_by_position,
    name_generated_roles, name_loop_and_catch_bindings, name_prototype_methods,
    name_vm_sub_parameters, native_for_in_loops, nest_literals, optional_chains,
    place_key_declarations, rebuild_classes, reconstruct_loops, remove_carry_self_assignments,
    remove_dead_carries, remove_dead_ssa, remove_discarded_argument_reads, remove_empty_else_blocks,
    remove_needless_loop_labels, remove_noop_local_reads, remove_redundant_initializers,
    remove_unread_key_stores, remove_unused_keys, replace_vm_values, resugar_expressions,
    scalarize_literal_tuples, share_vm_sub_helpers, shorten_generated_names,
    simplify_readable_program_property_writes, simplify_vm_sub_bodies, split_loop_tests,
    split_single_reads, step_copied_counters, thread_parameters, tidy_empty_starts,
    tidy_expressions, tidy_final_expressions, unpack_fresh_helper_tuples, unwrap_host_globals,
    unwrap_single_pass_loops, split_block_key_webs,
};

/// Writes a stage's code under its file name when stage dumping is on.
pub type Dump<'d> = &'d dyn Fn(&'static str, &str) -> Result<(), String>;

/// A pass's result: the code it wrote, besides whatever it counts.
pub(crate) trait PassOutput {
    fn into_code(self) -> String;
}

impl PassOutput for String {
    fn into_code(self) -> String {
        self
    }
}

impl PassOutput for (String, usize) {
    fn into_code(self) -> String {
        self.0
    }
}

impl PassOutput for (String, usize, usize) {
    fn into_code(self) -> String {
        self.0
    }
}

/// Results that carry the code in a `code` field.
macro_rules! code_field {
    ($($result:path),* $(,)?) => {
        $(impl PassOutput for $result {
            fn into_code(self) -> String {
                self.code
            }
        })*
    };
}

code_field!(
    super::super::alias_ssa::AliasSsaResult,
    super::super::carry_join::CarryJoinResult,
    super::super::literal_ssa::LiteralSsaResult,
    super::super::local_vars::LocalVars,
    super::super::name_loop_and_catch_bindings::LoopCatchNamesResult,
    super::super::loop_counters::LoopCountersResult,
    super::super::nest_literals::NestedLiterals,
    super::super::loop_tests::SplitLoopTests,
    super::super::tidy_expressions::TidyExpressionsResult,
    super::super::property_writes::PropertyWriteCleanupOutcome,
);

/// The code a stage wrote, or its error after `context`.
fn stage<O: PassOutput, E: std::fmt::Display>(context: &str, result: Result<O, E>) -> Result<String, String> {
    result.map(PassOutput::into_code).map_err(|error| format!("{context}: {error}"))
}

/// The cleanup stages one function goes through (`01-…13b-`): `fold_strings`
/// folds VM string-decoder calls whose arguments became literals.
pub fn clean_function(code: String, fold_strings: &dyn Fn(String) -> String, dump_stage: Dump) -> Result<String, String> {
    let code = ssa_and_carries(code, dump_stage)?;
    let code = vm_helpers(code, dump_stage)?;
    let code = dead_code_and_loops(code, dump_stage)?;
    let code = helper_expansion(code, fold_strings, dump_stage)?;
    let code = forwarding(code, dump_stage)?;
    let code = final_tidy(code, fold_strings, dump_stage)?;
    Ok(code)
}

/// 01-…09-: SSA copies and literals, property writes, carries and branch values.
fn ssa_and_carries(code: String, dump_stage: Dump) -> Result<String, String> {
    let code = match remove_redundant_initializers(&code) {
        Ok(cleaned) => cleaned.code,
        Err(error) => {
            eprintln!("function cleanup skipped redundant initializers: {error}");
            code
        }
    };
    dump_stage("01-redundant-initializers.js", &code)?;
    let mut code = code;
    for _ in 0..4 {
        match inline_adjacent_ssa(&code) {
            Ok(cleaned) => {
                code = cleaned.code;
                if cleaned.inlined_bindings == 0 {
                    break;
                }
            }
            Err(error) => {
                eprintln!("function cleanup skipped adjacent SSA: {error}");
                break;
            }
        }
    }
    dump_stage("02-adjacent-ssa.js", &code)?;
    let code = match collapse_literal_branch_joins(&code) {
        Ok(cleaned) => cleaned.code,
        Err(error) => {
            eprintln!("function cleanup skipped literal branch joins: {error}");
            code
        }
    };
    dump_stage("03-literal-branch-joins.js", &code)?;
    let code = stage("literal SSA", inline_literal_ssa(&code))?;
    dump_stage("04-literal-ssa.js", &code)?;
    let code = stage("readable property writes", simplify_readable_program_property_writes(&code))?;
    dump_stage("06-readable-property-writes.js", &code)?;
    let code = stage("cannot inline carry joins", inline_single_use_carry_joins(&code))?;
    dump_stage("07-carry-joins.js", &code)?;
    let mut code = code;
    // Coalescing one branch join exposes the copy immediately above it. The
    // bounded repeat collapses those chains without making a single pass
    // reason across unrelated branches or loop backedges.
    for _ in 0..4 {
        let result = coalesce_carry_copies(&code)
            .map_err(|error| format!("cannot coalesce carry copies: {error}"))?;
        code = result.code;
        if result.removed_carries == 0 {
            break;
        }
    }
    dump_stage("07b-carry-coalesced.js", &code)?;
    let code = stage("cannot remove carry self-assignments", remove_carry_self_assignments(&code))?;
    dump_stage("07b2-carry-self-assignments.js", &code)?;
    let mut code = code;
    for _ in 0..4 {
        let result = remove_dead_carries(&code)
            .map_err(|error| format!("cannot remove dead carries: {error}"))?;
        code = result.code;
        if result.removed_carries == 0 {
            break;
        }
    }
    dump_stage("07c-dead-carries.js", &code)?;
    let mut code = code;
    for _ in 0..4 {
        let result = fold_branch_values(&code)
            .map_err(|error| format!("cannot fold branch carry values: {error}"))?;
        code = result.code;
        if result.replaced_carries == 0 {
            break;
        }
    }
    dump_stage("07d-branch-values.js", &code)?;
    let code = stage("cannot inline SSA aliases", inline_alias_ssa(&code))?;
    dump_stage("08-alias-ssa.js", &code)?;
    let mut code = code;
    for _ in 0..4 {
        let cleaned = inline_adjacent_ssa(&code)
            .map_err(|error| format!("cannot inline exposed adjacent SSA: {error}"))?;
        code = cleaned.code;
        if cleaned.inlined_bindings == 0 {
            break;
        }
    }
    dump_stage("09-adjacent-after-alias.js", &code)?;
    Ok(code)
}

/// 11-…11b5-: VM subroutine parameters, shared helpers, wrappers and tuples.
fn vm_helpers(code: String, dump_stage: Dump) -> Result<String, String> {
    let code = name_vm_sub_parameters(&code)
        .map_err(|error| format!("cannot name VM subroutine parameters: {error}"))?;
    dump_stage("11-vm-sub-parameters.js", &code)?;
    let shared = share_vm_sub_helpers(&code)
        .map_err(|error| format!("cannot share VM subroutine helpers: {error}"))?;
    debug_assert!(shared.removed == 0 || shared.code.len() < code.len());
    let code = shared.code;
    dump_stage("11b-shared-vm-helpers.js", &code)?;
    let simplified = simplify_vm_sub_bodies(&code)
        .map_err(|error| format!("cannot simplify VM subroutines: {error}"))?;
    let code = simplified.code;
    dump_stage("11b1-simplified-vm-subs.js", &code)?;
    let before_wrapper_bytes = code.len();
    let mut code = code;
    let mut wrapper_inlined = 0;
    for _ in 0..4 {
        let inlined = inline_vm_sub_wrappers(&code)
            .map_err(|error| format!("cannot inline VM wrappers: {error}"))?;
        wrapper_inlined += inlined.removed;
        if inlined.code == code {
            break;
        }
        code = inlined.code;
    }
    let after_inline_bytes = code.len();
    dump_stage("11b2-inline-vm-wrappers.js", &code)?;
    let scalarized = scalarize_literal_tuples(&code)
        .map_err(|error| format!("cannot scalarize VM tuples: {error}"))?;
    debug_assert!(scalarized.removed == 0 || scalarized.code.len() < after_inline_bytes);
    debug_assert!(wrapper_inlined == 0 || scalarized.code.len() < before_wrapper_bytes);
    let code = scalarized.code;
    dump_stage("11b3-scalar-tuples.js", &code)?;
    let mut code = stage("cannot inline scalarized SSA aliases", inline_alias_ssa(&code))?;
    for _ in 0..4 {
        let cleaned = inline_adjacent_ssa(&code)
            .map_err(|error| format!("cannot inline scalarized adjacent SSA: {error}"))?;
        code = cleaned.code;
        if cleaned.inlined_bindings == 0 {
            break;
        }
    }
    dump_stage("11b4-ssa-after-tuples.js", &code)?;
    let unpacked = unpack_fresh_helper_tuples(&code)
        .map_err(|error| format!("cannot unpack fresh VM tuples: {error}"))?;
    debug_assert!(unpacked.collapsed == 0 || unpacked.code.len() < code.len());
    let code = unpacked.code;
    dump_stage("11b5-unpacked-tuples.js", &code)?;
    Ok(code)
}

/// 11c-…11h-: dead SSA and slot stores, loops and their counters, literal fills, names and host globals.
fn dead_code_and_loops(code: String, dump_stage: Dump) -> Result<String, String> {
    let dead = remove_dead_ssa(&code)
        .map_err(|error| format!("cannot remove dead SSA temporaries: {error}"))?;
    debug_assert!(dead.removed == 0 || dead.code.len() < code.len());
    let code = dead.code;
    dump_stage("11c-dead-ssa.js", &code)?;
    let dead_keys = remove_unread_key_stores(&code)
        .map_err(|error| format!("cannot remove unread VM slot stores: {error}"))?;
    debug_assert!(dead_keys.removed_stores == 0 || dead_keys.code.len() < code.len());
    let mut code = dead_keys.code;
    dump_stage("11c3-dead-key-stores.js", &code)?;
    for _ in 0..4 {
        let dead = remove_dead_ssa(&code)
            .map_err(|error| format!("cannot remove SSA exposed by dead slots: {error}"))?;
        code = dead.code;
        let (without_reads, noops) = remove_noop_local_reads(&code)
            .map_err(|error| format!("cannot remove reads exposed by dead slots: {error}"))?;
        code = without_reads;
        let keys = remove_unread_key_stores(&code)
            .map_err(|error| format!("cannot remove newly unread VM slots: {error}"))?;
        code = keys.code;
        // Helper and tuple cleanup after stage 10 leaves carries that
        // only relay values nobody reads.
        let carries = remove_dead_carries(&code)
            .map_err(|error| format!("cannot remove carries exposed by dead slots: {error}"))?;
        code = carries.code;
        if dead.removed == 0
            && noops == 0
            && keys.removed_stores == 0
            && carries.removed_carries == 0
        {
            break;
        }
    }
    dump_stage("11c4-dead-after-keys.js", &code)?;
    let code = stage("cannot remove unused argument reads", remove_discarded_argument_reads(&code))?;
    dump_stage("11c5-argument-reads.js", &code)?;
    let code = stage("cannot remove exposed carry self-assignments", remove_carry_self_assignments(&code))?;
    let code = stage("cannot remove empty else blocks", remove_empty_else_blocks(&code))?;
    dump_stage("11d-empty-else.js", &code)?;
    // A loop nothing jumps back to runs its body once: it goes, so the
    // loop around it shows its shape.
    let code = stage("cannot unwrap single-pass loops", unwrap_single_pass_loops(&code))?;
    let protected_keys = collect_key_names(&code);
    let code = stage("cannot reconstruct loops", reconstruct_loops(&code, &protected_keys))?;
    dump_stage("11d2-reconstructed-loops.js", &code)?;
    let code = stage("cannot lift loop counters", lift_loop_counters(&code))?;
    dump_stage("11d3-lifted-loop-counters.js", &code)?;
    let fills = fold_literal_fills(&code)
        .map_err(|error| format!("cannot fold literal fills: {error}"))?;
    let code = fills.code;
    dump_stage("11e2-fold-literal-fills.js", &code)?;
    // Loop tests computed in steps become statements while their
    // temporaries still carry generated names.
    let code = stage("cannot split early loop tests", split_loop_tests(&code))?;
    dump_stage("11e3-split-loop-tests.js", &code)?;
    let single = inline_single_use_literals(&code)
        .map_err(|error| format!("cannot inline single-use literals: {error}"))?;
    let code = single.code;
    dump_stage("11f-single-use-literals.js", &code)?;
    let names = name_loop_and_catch_bindings(&code)
        .map_err(|error| format!("cannot name loop and catch bindings: {error}"))?;
    let code = names.code;
    dump_stage("11g-loop-catch-names.js", &code)?;
    // `host["Name"]` becomes the global `Name` before forwarding, so the
    // standard built-ins it names read as stable values.
    let globals = unwrap_host_globals(&code)
        .map_err(|error| format!("cannot unwrap host globals: {error}"))?;
    let code = globals.code;
    dump_stage("11h-host-globals.js", &code)?;
    Ok(code)
}

/// 11h0-…11h5-: helper bodies tidied, small and tuple helpers expanded.
fn helper_expansion(code: String, fold_strings: &dyn Fn(String) -> String, dump_stage: Dump) -> Result<String, String> {
    // Helper bodies still hold their own temporaries and spell method
    // calls as `o[k].call(o, …)`; forward and fold them first so the
    // helpers are one line and can expand.
    let forwarded = forward_values(&code)
        .map_err(|error| format!("cannot forward helper bodies: {error}"))?;
    let resugared = resugar_expressions(&forwarded.code)
        .map_err(|error| format!("cannot resugar helper bodies: {error}"))?;
    let code = resugared.code;
    dump_stage("11h0-resugared-helpers.js", &code)?;
    // Small and tuple expansions feed each other (a tuple slot becomes a
    // stable argument; a held tuple read inside another expanded call
    // waits for that call to become statements): alternate until both
    // settle.
    let mut code = code;
    for _ in 0..12 {
        // A statement helper called inside an expression becomes a
        // declaration of its own first.
        let hoisted = hoist_helper_calls(&code)
            .map_err(|error| format!("cannot hoist helper calls: {error}"))?;
        let small = inline_small_helpers(&hoisted.code)
            .map_err(|error| format!("cannot expand small helpers: {error}"))?;
        let tuples = inline_tuple_helpers(&small.code)
            .map_err(|error| format!("cannot expand tuple helpers: {error}"))?;
        code = tuples.code;
        if small.expanded == 0 && tuples.expanded == 0 {
            break;
        }
    }
    // Expanded helpers hand the string decoder literal arguments.
    let code = fold_strings(code);
    dump_stage("11h1-expanded-helpers.js", &code)?;
    // Expanded helpers leave `[]` literals filled by index.
    let fills = fold_literal_fills(&code)
        .map_err(|error| format!("cannot fold expanded literal fills: {error}"))?;
    // Literals filled out of order or nested one in another build in one
    // piece where they are used.
    let code = stage("cannot nest expanded literals", nest_literals(&fills.code))?;
    dump_stage("11h3-expanded-literal-fills.js", &code)?;
    // Helper expansion turns a counter's update into plain arithmetic, so a
    // second lifting round sees counters the first one could not.
    let code = stage("cannot lift expanded loop counters", lift_loop_counters(&code))?;
    let code = stage("cannot name expanded loop bindings", name_loop_and_catch_bindings(&code))?;
    dump_stage("11h4-lifted-loop-counters.js", &code)?;
    // Forwarding reads through `(0, e)` wrappers as opaque: drop the
    // meaningless ones first.
    let code = stage("cannot tidy expressions before forwarding", tidy_expressions(&code))?;
    dump_stage("11h5-tidied.js", &code)?;
    expanded_loops(code, dump_stage)
}

/// 11i-…11p-: forwarding, branch values, local vars, derived values and the remaining helpers.
fn forwarding(code: String, dump_stage: Dump) -> Result<String, String> {
    let code = forward_and_fold(code, dump_stage)?;
    let code = merge_and_split(code, dump_stage)?;
    late_helpers(code, dump_stage)
}

/// 11i-…11l-: forwarding with branch values, then forwarding and its dead
/// cleanup in turn until the cleanup finds nothing.
fn forward_and_fold(code: String, dump_stage: Dump) -> Result<String, String> {
    let forwarded = forward_values(&code)
        .map_err(|error| format!("cannot forward late generated values: {error}"))?;
    let code = forwarded.code;
    dump_stage("11i-forwarded-values.js", &code)?;
    let resugared = resugar_expressions(&code)
        .map_err(|error| format!("cannot resugar expressions: {error}"))?;
    let code = resugared.code;
    dump_stage("11i2-resugared.js", &code)?;
    let joins = fold_branch_values(&code)
        .map_err(|error| format!("cannot fold late branch values: {error}"))?;
    let code = joins.code;
    dump_stage("11j-late-branch-values.js", &code)?;
    let forwarded = forward_values(&code)
        .map_err(|error| format!("cannot forward late branch values: {error}"))?;
    let code = forwarded.code;
    dump_stage("11k-forwarded-branch-values.js", &code)?;
    // The forward loop and the dead cleanup after it feed each other: run
    // them together until the cleanup finds nothing.
    let mut code = code;
    for _ in 0..3 {
        // Forwarding builds new method calls and tests out of the values it
        // moves, and folding them leaves values single-use: alternate until
        // neither changes anything.
        for _ in 0..8 {
            let (next, changed) = forwarded_round(&code)?;
            code = next;
            if !changed {
                break;
            }
        }
        dump_stage("11k3-forwarded-again.js", &code)?;
        let (cleaned, removed) = dead_after_forwarding(&code)?;
        code = cleaned;
        dump_stage("11l-dead-after-forwarding.js", &code)?;
        // Removed stores and reads can leave arms that only assign a
        // carry: go around once more if this cleanup changed anything.
        if removed == 0 {
            break;
        }
    }
    Ok(code)
}

/// One round of folding what forwarding exposed: resugaring, branch values,
/// diamonds and tails, exits and literal fills, then one forwarding pass.
/// Returns the code and whether anything changed.
fn forwarded_round(code: &str) -> Result<(String, bool), String> {
    let resugared = resugar_expressions(code)
        .map_err(|error| format!("cannot resugar forwarded expressions: {error}"))?;
    // Forwarding leaves arms that only assign a carry: fold them so
    // the next round can move the resulting `const`.
    let joins = fold_branch_values(&resugared.code)
        .map_err(|error| format!("cannot fold forwarded branch values: {error}"))?;
    let multi = fold_multi_branch_values(&joins.code)
        .map_err(|error| format!("cannot fold multi-carry branches: {error}"))?;
    // A diamond storing into a carry written elsewhere too is
    // still one conditional store.
    let (diamonds, stores) = fold_plain_branch_assignments(&multi.code)
        .map_err(|error| format!("cannot fold assignment diamonds: {error}"))?;
    // The statements every arm ends with run once after the arms,
    // where forwarding can see them.
    let (diamonds, tails) = hoist_branch_tails(&diamonds)
        .map_err(|error| format!("cannot hoist branch tails: {error}"))?;
    let flat = flatten_exits(&diamonds)
        .map_err(|error| format!("cannot flatten exits: {error}"))?;
    // Forwarded store values can now join the literal they fill.
    let fills = fold_literal_fills(&flat.code)
        .map_err(|error| format!("cannot fold forwarded literal fills: {error}"))?;
    let forwarded = forward_values_once(&fills.code)
        .map_err(|error| format!("cannot forward resugared values: {error}"))?;
    let changed = !(resugared.unwrapped == 0
        && joins.replaced_carries == 0
        && multi.replaced_carries == 0
        && flat.flattened + flat.sunk + flat.hoisted == 0
        && stores == 0
        && tails == 0
        && fills.folded_stores == 0
        && forwarded.forwarded == 0);
    Ok((forwarded.code, changed))
}

/// Values, carries, slot stores and argument reads that forwarding left
/// unread. Returns the code and how much went.
fn dead_after_forwarding(code: &str) -> Result<(String, usize), String> {
    // Values read only by deleted decoys are dead now.
    let dead = remove_dead_ssa(code)
        .map_err(|error| format!("cannot remove SSA left by decoys: {error}"))?;
    let carries = remove_dead_carries(&dead.code)
        .map_err(|error| format!("cannot remove carries left by decoys: {error}"))?;
    // Stores whose reads all took the stored value are dead now.
    let keys = remove_unread_key_stores(&carries.code)
        .map_err(|error| format!("cannot remove slot stores left by forwarding: {error}"))?;
    // A dropped `$KEY = arguments[n]` store leaves its read behind.
    let (cleaned, removed_reads) = remove_discarded_argument_reads(&keys.code)
        .map_err(|error| format!("cannot remove argument reads left by forwarding: {error}"))?;
    Ok((cleaned, dead.removed + carries.removed_carries + keys.removed_stores + removed_reads))
}

/// 11l2-…11o-: local vars, then derived values merged and single reads split
/// for forwarding to place.
fn merge_and_split(mut code: String, dump_stage: Dump) -> Result<String, String> {
    // Slots nothing reads go; a var its function stores first is a let.
    code = clean_local_vars(&code)
        .map_err(|error| format!("cannot clean local vars: {error}"))?
        .code;
    dump_stage("11l2-local-vars.js", &code)?;
    // Property reads go to their reads before merging, which would make
    // their objects change under them.
    code = forward_values_and_property_reads(&code)
        .map_err(|error| format!("cannot forward property reads before merging: {error}"))?
        .code;
    // A value derived from a local whose old value is dead is that local's
    // next value: merge the chains the VM split one variable into.
    code = merge_until_settled(code, "cannot merge derived values")?;
    dump_stage("11m-merged-derived-values.js", &code)?;
    // A value only the next store reads becomes its own binding for
    // forwarding to place; what stays merges back.
    let mut code = code;
    for _ in 0..6 {
        let split = split_single_reads(&code)
            .map_err(|error| format!("cannot split single reads: {error}"))?;
        if split.merged == 0 {
            break;
        }
        code = forward_values_and_property_reads(&split.code)
            .map_err(|error| format!("cannot forward split values: {error}"))?
            .code;
    }
    let code = merge_until_settled(code, "cannot merge split values")?;
    dump_stage("11o-split-single-reads.js", &code)?;
    Ok(code)
}

/// 11p0-…11p-: the helpers forwarding left small enough, then the rest
/// expanded exactly, each round folded and merged.
fn late_helpers(code: String, dump_stage: Dump) -> Result<String, String> {
    // Forwarding leaves some helper bodies one expression or a few
    // statements that the first expansion could not take (their own
    // `let`s, values computed in steps): expand them now, and fold the
    // strings their literal arguments decode to.
    let mut code = code;
    for _ in 0..4 {
        let small = inline_small_helpers(&code)
            .map_err(|error| format!("cannot expand late small helpers: {error}"))?;
        code = small.code;
        if small.expanded == 0 {
            break;
        }
    }
    // Whatever helpers are left expand exactly as their calls run, and
    // forwarding folds the temporaries that leaves.
    // Folding the expanded values can expose more calls (a helper's result
    // read through a tuple): go around until a round expands nothing.
    for _ in 0..4 {
        let mut remaining = 0;
        // A round may only move a call closer to a statement of its own (a
        // loop test, a sequence, a choice): go on while either step acts.
        for _ in 0..8 {
            let hoisted = hoist_remaining_helper_calls(&code)
                .map_err(|error| format!("cannot hoist remaining helper calls: {error}"))?;
            let exact = inline_remaining_helpers(&hoisted.code)
                .map_err(|error| format!("cannot expand remaining helpers: {error}"))?;
            code = exact.code;
            remaining += exact.expanded;
            if exact.expanded == 0 && hoisted.expanded == 0 {
                break;
            }
        }
        dump_stage("11p0-remaining-helpers.js", &code)?;
        if remaining > 0 {
            code = nest_literals(&code)
                .map_err(|error| format!("cannot nest expanded helper literals: {error}"))?
                .code;
            code = tidy_expressions(&code)
                .map_err(|error| format!("cannot tidy expanded helpers: {error}"))?
                .code;
            code = forward_values_and_property_reads(&code)
                .map_err(|error| format!("cannot forward expanded helper values: {error}"))?
                .code;
            dump_stage("11p1-forwarded-remaining-helpers.js", &code)?;
            code = merge_until_settled(code, "cannot merge expanded helper values")?;
        }
        if remaining == 0 {
            break;
        }
    }
    dump_stage("11p-late-helpers.js", &code)?;
    Ok(code)
}

/// 11h6-: loops the expanded helpers gave a plain shape. A loop whose test
/// went through a tuple helper (`let [t, i] = $VMHELPER(…)`) kept its
/// `vm_loop_*` form at `11d2`; the expanded helper leaves the test plain
/// `let`s, so the same reconstruction applies now, before forwarding
/// rewrites the arms.
fn expanded_loops(code: String, dump_stage: Dump) -> Result<String, String> {
    if !code.contains("vm_loop_") {
        return Ok(code);
    }
    let code = stage("cannot unwrap late single-pass loops", unwrap_single_pass_loops(&code))?;
    let protected_keys = collect_key_names(&code);
    let (code, rebuilt) = reconstruct_loops(&code, &protected_keys)
        .map_err(|error| format!("cannot reconstruct late loops: {error}"))?;
    let code = if rebuilt > 0 {
        let code = stage("cannot lift late loop counters", lift_loop_counters(&code))?;
        stage("cannot split late loop tests", split_loop_tests(&code))?
    } else {
        code
    };
    dump_stage("11h6-expanded-loops.js", &code)?;
    Ok(code)
}

/// Merges derived values until a round merges none; `context` names the
/// step in the error.
fn merge_until_settled(mut code: String, context: &str) -> Result<String, String> {
    for _ in 0..24 {
        let merged = merge_derived_values(&code).map_err(|error| format!("{context}: {error}"))?;
        code = merged.code;
        if merged.merged == 0 {
            break;
        }
    }
    Ok(code)
}

/// 13b-: the final tidy, loop restructuring and declaration placement.
fn final_tidy(code: String, fold_strings: &dyn Fn(String) -> String, dump_stage: Dump) -> Result<String, String> {

    let code = fold_strings(code);
    // Forwarding leaves slots whose reads all moved: their stores go.
    let code = stage("cannot clean late local vars", clean_local_vars(&code))?;
    let code = stage("cannot nest late literals", nest_literals(&code))?;
    // Late forwarding leaves `o.m.call(o, …)` the earlier resugaring
    // already wrote as `o.m(…)`.
    let code = stage("cannot resugar late values", resugar_expressions(&code))?;
    // A split comma test's counter copy reads as the counter, so the tidy
    // rebuilds the counted loop.
    let code = stage("cannot merge loop counter copies", merge_loop_counter_copies(&code))?;
    // Lowered `?.` chains go back to `?.`; the tidy drops the parentheses
    // the conditionals needed.
    let code = stage("cannot restore optional chains", optional_chains(&code))?;
    // The final tidy is per function: it runs here, on each thread.
    let code = stage("cannot tidy expressions", tidy_final_expressions(&code))?;
    // The tidy writes each arm's counter steps alike: shared tails move out.
    let (code, hoisted) = hoist_branch_tails(&code).map_err(|error| format!("cannot hoist tidied branch tails: {error}"))?;
    // A moved step folds into the store after it.
    let code = if hoisted > 0 {
        tidy_final_expressions(&code).map_err(|error| format!("cannot tidy hoisted tails: {error}"))?.code
    } else {
        code
    };
    // The VM's key-snapshot enumeration goes back to a native `for…in`.
    let code = stage("cannot restore for-in loops", native_for_in_loops(&code))?;
    // A block that writes a reused slot before reading it holds a value of
    // its own: its own binding of the slot's name.
    let code = stage("cannot split key webs", split_block_key_webs(&code))?;
    // Each slot variable is declared once, where its uses are.
    let code = stage("cannot place key declarations", place_key_declarations(&code))?;
    // Loop labels the restructured jumps no longer need go.
    let code = stage("cannot drop loop labels", remove_needless_loop_labels(&code))?;
    // A body's closing `return void 0;` is implicit.
    let code = stage("cannot drop trailing returns", drop_trailing_returns(&code))?;
    dump_stage("13b-tidy-expressions.js", &code)?;
    Ok(code)
}

/// The whole program's stages (`12-…13e-`), once every function is cleaned
/// and linked: names, classes and arrows, VM values, and the late tidies.
/// `function_names` numbers each function's namespace.
pub fn finish_program(
    code: String,
    function_names: &[(String, usize)],
    native_functions: &[String],
    source: &str,
    dump_stage: Dump,
) -> Result<String, String> {
    let code = shorten_generated_names(&code, &function_names)
        .map_err(|error| format!("cannot shorten generated names: {error}"))?;
    dump_stage("12-short-generated-names.js", &code)?;
    let roles = name_generated_roles(&code)
        .map_err(|error| format!("cannot name generated roles: {error}"))?;
    let code = roles.code;
    dump_stage("13c-role-names.js", &code)?;
    // Constructor and method threads go back to `class`: a whole-program
    // pass, since the methods are other threads.
    let code = stage("cannot rebuild classes", rebuild_classes(&code))?;
    dump_stage("13c2-classes.js", &code)?;
    // Single-use callbacks become arrows where they are passed: also
    // whole-program, since the callbacks are other threads.
    let code = stage("cannot write arrow callbacks", arrow_callbacks(&code))?;
    dump_stage("13c3-arrows.js", &code)?;
    // Threads read `arguments[n]`: they get parameters, and function
    // values called with no receiver are called directly.
    let code = stage("cannot name thread parameters", thread_parameters(&code))?;
    dump_stage("13c4-parameters.js", &code)?;
    // The VM values the program still names: the native catalogue and
    // the root's empty result.
    let code = stage("cannot replace vm values", replace_vm_values(&code, native_functions, source))?;
    // A slot that only holds a global's value (`Array.prototype.push`,
    // `window`, a `RegExp`) is read as that value.
    let code = stage("cannot inline constant slots", inline_constant_slots(&code))?;
    // Slots nothing reads or writes anywhere go.
    let code = stage("cannot remove unused keys", remove_unused_keys(&code))?;
    let code = stage("cannot name bindings by position", name_bindings_by_position(&code))?;
    dump_stage("13d-positional-names.js", &code)?;
    // `[].push` is `Array.prototype.push`, `{}.hasOwnProperty`
    // `Object.prototype.hasOwnProperty`.
    let code = stage("cannot name prototype methods", name_prototype_methods(&code))?;
    // A property read held for the next statement is read there.
    let code = stage("cannot inline path temporaries", inline_path_temporaries(&code))?;
    // `let q = [x, void 0]; q[1] = y;` → `let q = [x, y];`.
    let code = stage("cannot fold array fills", fold_array_fills(&code))?;
    // `let a = x | 0; a = a | y << 1; … use(a)`: one expression at the use.
    let code = stage("cannot fold bit accumulators", fold_bit_accumulators(&code))?;
    // A counter stepped through the copy its body takes is stepped itself.
    let code = stage("cannot step copied counters", step_copied_counters(&code))?;
    // `!(a === b)` → `a !== b`; `!(i < n)` → `i >= n` over numbers that
    // are never NaN.
    let code = stage("cannot flip negated comparisons", flip_negated_comparisons(&code))?;
    // `let x = void 0;` → `let x;`, and `let x; x = e;` → `let x = e;`.
    let code = stage("cannot tidy empty starts", tidy_empty_starts(&code))?;
    // A literal read once by the next statement is built there.
    let code = stage("cannot inline adjacent literals", inline_adjacent_literals(&code))?;
    // `let a;\nlet b;` → `let a, b;`, once nothing else merges a `let`.
    let code = stage("cannot join let runs", join_let_runs(&code))?;
    // Last: `o["k"]` → `o.k`, `{ ["k"]: v }` → `{ k: v }`. Earlier passes
    // recognise code by its bracketed spelling, so this stays the final
    // stage.
    let code = stage("cannot dot property names", dotted_properties(&code))?;
    // A function declaration (and its alias) between two property stores to
    // the same object moves after the run, so the stores read together.
    let code = stage("cannot move declarations out of store runs", move_store_declarations(&code))?;
    dump_stage("13f-store-declarations.js", &code)?;
    Ok(code)
}

/// Every `$KEY_n` token in the linked program, treated as a shared VM slot
/// whose declaration must not be lifted into a loop header.
fn collect_key_names(source: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for (start, _) in source.match_indices("$KEY_") {
        let tail = &source[start..];
        let end = tail[5..]
            .bytes()
            .position(|byte| !byte.is_ascii_digit())
            .map_or(tail.len(), |at| at + 5);
        names.insert(tail[..end].to_string());
    }
    names
}
