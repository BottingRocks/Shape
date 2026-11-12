//! Static kernel facts, proofs, and projections.
//!
//! These modules describe what can be established from source and pipeline
//! reports.  They do not own execution, tracing, or dispatch behavior.

pub mod bundle;
pub mod config;
pub mod decoder_proof;
pub mod layout;
pub mod properties;
pub mod roles;

// The WASM boundary and its helpers live together under `kernel/wasm/`. The
// boundary keeps the `kernel::wasm` path; the dispatch adapter, interpreter
// lift, and low-level module inspector keep their established paths as
// re-exports:
//
//   kernel::wasm         -> src/kernel/wasm/mod.rs
//   kernel::wasm_dispatch-> src/kernel/wasm/dispatch.rs
//   kernel::wasm_lift    -> src/kernel/wasm/lift.rs
//   kernel::wasm_module  -> src/kernel/wasm/module.rs
pub mod wasm;
pub use wasm::dispatch as wasm_dispatch;
pub(crate) use wasm::lift as wasm_lift;
pub(crate) use wasm::module as wasm_module;
