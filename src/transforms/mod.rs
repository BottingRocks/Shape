//! Source deobfuscation and post-emission JavaScript cleanup.
//!
//! Two independent pipelines live here and stay distinct in code and
//! documentation:
//!
//! - [`deob`] standardizes raw source through CLEAN → RENAME → FIND → EXTRACT
//!   and produces the shared `PipelineReport`.
//! - [`emitted_js`] runs Oxc passes over the JavaScript the emitter produced.
//!   It is the implementation behind the public `shape::emitted_cleanup` path.
//!
//! [`diagnostics`] holds the typed per-stage findings the source pipelines
//! record. `README.md` is the overview; the detailed source-pass material is in
//! `deob/README.md` and the cleanup-pass material is in `emitted_js/README.md`.

pub(crate) mod deob;
pub mod diagnostics;
pub mod emitted_js;
