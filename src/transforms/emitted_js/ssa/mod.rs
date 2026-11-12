//! Forwarding and folding generated `$SSA_`/slot values.

pub(crate) mod adjacent_ssa;
pub(crate) mod alias_ssa;
pub(crate) mod literal_ssa;
pub(crate) mod snapshot_ssa;
pub(crate) mod snapshot_copies;
pub(crate) mod slot_forward;
pub(crate) mod argument_snapshots;
pub(crate) mod forward_values;
pub(crate) mod merge_derived;
pub(crate) mod local_values;
pub(crate) mod dead_ssa;
pub(crate) mod constant_values;
