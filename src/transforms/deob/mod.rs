//! Source-preparation pipeline and its passes.
//!
//! [`pipeline`] owns the CLEAN → RENAME → FIND → EXTRACT stages and produces
//! the shared `PipelineReport` that every later area consumes. The remaining
//! modules are the passes it drives:
//!
//! - [`naming`] renames bindings to canonical role names (RENAME).
//! - [`native_cleanup`] folds provable native/built-in calls and aliases (CLEAN).
//! - [`string_decode`] decodes supported string/array constructions (CLEAN).
//! - [`operation_table`] locates the numeric operation table (FIND/EXTRACT).
//! - [`prototype_aliases`] resolves prototype-method aliases (CLEAN).
//! - [`variable_declarations`] normalizes declaration shapes (CLEAN).
//! - [`wasm_bindings`] extracts embedded WASM modules and their bindings (EXTRACT).
//!
//! This is source deobfuscation only. It never touches the JavaScript the
//! emitter produces; that is [`crate::transforms::emitted_js`]. See `README.md`
//! here for the per-pass material and `docs/deob/deobfuscation.md` for the
//! cross-module view.

pub(crate) mod naming;
pub(crate) mod operation_table;
pub(crate) mod pipeline;
pub(crate) mod prototype_aliases;
pub(crate) mod variable_declarations;

pub mod native_cleanup;
pub mod string_decode;
pub mod wasm_bindings;
