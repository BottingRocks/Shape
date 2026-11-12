//! Public tracing components.
//!
//! The symbolic executor/emitter and the plain-text observation renderer are
//! intentionally separate event models. This facade groups their canonical
//! implementations without adapting or merging either model.

mod predicate;
pub mod scope;
pub mod symbolic;
pub mod text;
