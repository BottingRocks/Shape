//! Native trace commands and JSON output. The walking and linking live in
//! [`shape::trace`]; these modules parse flags and print.
pub mod commands;

pub(crate) use commands::run;
pub(crate) use shape::trace::{corpus, inputs, read_kernel_artifact};
