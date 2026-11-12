//! Signals: everything about the signals a Shape program collects.
//!
//! - [`codec`]: the wire writers of one signal (keystream, number/string/byte
//!   writers, token hash).
//! - [`payload`]: the report payload around the signals (framing, cipher,
//!   encode/decode) and the CLI commands built on it.
//!
//! - [`extract`]: the structural program readers (bindings, descriptor
//!   scanner, wiring, encoding, signal layouts, generator inputs, contracts).
//! - [`analysis`]: the per-seed signal schema (`analyze_program_signals`), its
//!   recognizers, flag decoders and the finder roles in [`analysis::roles`].
//! - [`catalog`]: one file per signal (recognizer, binder and its approved
//!   field card `CARD`), the canonical identity
//!   registry of all 93 [`catalog::SIGNALS`].
//! - [`schema`]: the shared report types (spans, bindings, confidence, the
//!   program-properties report).
//! - [`docs`]: the `docs/signals/` pages rendered from the cards
//!   (`shape signals docs <dir>`).
//! - [`decode`]: reusable source-compiled named signal decode artifacts and typed captures.

pub mod analysis;
pub mod catalog;
pub mod codec;
pub mod decode;
pub mod docs;
pub mod extract;
pub mod inspect;
pub(crate) mod flags;
pub mod payload;
pub mod schema;
pub(crate) mod timing;
pub mod wire;
