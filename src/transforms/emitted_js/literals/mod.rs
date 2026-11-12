//! Array and object literals and property writes.

pub(crate) mod property_writes;
pub(crate) mod dense_arrays;
pub(crate) mod array_initializers;
pub(crate) mod array_format;
pub(crate) mod array_fills;
pub(crate) mod fold_literal_fills;
pub(crate) mod move_store_declarations;
pub(crate) mod nest_literals;
pub(crate) mod point_literals;
pub(crate) mod adjacent_literals;
pub(crate) mod inline_single_use_literals;
