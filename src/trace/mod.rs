//! Trace orchestration over the symbolic tracer: one thread, one dispatch, the
//! reachable program, and corpus fingerprints.
//!
//! The `shape trace` commands parse flags and print; the walking, linking and
//! comparison live here so the SDK can call them without the command line.
mod artifact;
pub mod cell_contents;
pub mod corpus;
pub mod inputs;
pub mod program;
pub mod program_nest;
pub mod return_shapes;
pub mod thread;

pub use artifact::read_kernel_artifact;
