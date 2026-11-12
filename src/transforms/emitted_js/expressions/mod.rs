//! Expression tidying.

pub(crate) mod tidy_expressions;
pub(crate) mod negated_comparisons;
pub(crate) mod numeric_coercions;
// No stage runs it since the per-thread pipeline (it changed no thread).
#[allow(dead_code)]
pub(crate) mod method_calls;
pub(crate) mod optional_chains;
pub(crate) mod nullish_syntax;
pub(crate) mod null_flags;
pub(crate) mod discarded_values;
pub(crate) mod bit_accumulators;
pub(crate) mod path_temporaries;
pub(crate) mod constant_slots;
pub(crate) mod prototype_methods;
pub(crate) mod empty_starts;
pub(crate) mod let_runs;
pub(crate) mod trailing_returns;
