//! Acquisition of Shape-style async launchers and the seed kernels they load.
//!
//! These modules implement the `shape seeds` commands. The crate is split so
//! that nothing which reads JavaScript touches the network and nothing which
//! touches the network reads JavaScript:
//!
//! * [`script`] — local, Oxc-based classification of a saved file as an async
//!   launcher (yielding the kernel URL it loads) or a raw kernel.
//! * [`seed`] — URL resolution and seed-token extraction with URL semantics.
//! * [`http`] — the [`http::HttpClient`] seam and its reqwest implementation.
//! * [`sites`] — the per-site registry of launcher entry points.
//! * [`store`] — `<seed>.async.js` / `<seed>.raw.js` siblings, never overwritten.
//! * [`collect`] — the launcher → kernel → store flow built from the above.
//! * [`cli`] — the in-process `shape seeds` command runner.

pub mod collect;
pub mod error;
pub mod http;
pub mod script;
pub mod seed;
pub mod sites;
pub mod store;

pub use error::{Error, Result, Role, TransportKind};
