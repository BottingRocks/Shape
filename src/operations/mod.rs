//! Canonical public modules for bounded operation analysis and projections.
//!
//! Each child keeps a distinct responsibility: catalogue inventory, template
//! admission, operand decoding, concrete execution, and CLI views.

pub mod catalogue;
pub mod classify;
pub mod executor;
pub mod graph;
pub mod operands;
pub(crate) mod table_scan;
pub mod templates;
pub mod view;
