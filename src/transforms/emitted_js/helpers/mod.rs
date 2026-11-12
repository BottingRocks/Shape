//! VM helper and tuple inlining.

pub(crate) mod vm_sub_dedupe;
pub(crate) mod vm_sub_parameters;
pub(crate) mod vm_sub_simplify;
pub(crate) mod inline_vm_sub;
pub(crate) mod expand_vm_sub;
pub(crate) mod inline_small_helpers;
pub(crate) mod inline_tuple_helpers;
pub(crate) mod runtime_helpers;
pub(crate) mod tuple_scalarize;
pub(crate) mod tuple_unpack;
