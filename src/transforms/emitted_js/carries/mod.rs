//! Loop and branch carries.

pub(crate) mod carry_join;
pub(crate) mod carry_coalesce;
pub(crate) mod carry_self_assign;
pub(crate) mod carry_lifetimes;
pub(crate) mod dead_carry;
pub(crate) mod equivalent_carry;
pub(crate) mod join_channels;
pub(crate) mod stable_channels;
#[cfg(test)]
pub(crate) mod carry_semantics_tests;
