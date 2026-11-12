//! Structural seed extraction.
//!
//! A captured seed is traced once with `shape trace program`, then [`pin`]
//! locates the few places that define the capture: the stream-cipher state and
//! its loop, the report route's filter and descriptor table, the constructor's
//! ordered keyset mapping, and the header prefix/chunk/`c`/`d`/`z` literals.
//! Every fact is proved structurally and keeps its byte span; nothing is
//! compared against a reviewed source template.
//!
//! The strict whole-body contract extractor and its `contracts/` JavaScript
//! templates were removed. They could only accept the exact emission they were
//! written from, which a traced seed rarely matches. [`pin`] covers the capture
//! path instead and writes one [`SeedLayout`] artifact.
mod ast;
mod pin;
mod types;

pub use pin::{
    find_seed_id, pin, pin_source, session_from_initializer, HeaderLayout, KeyedSignalLayout,
    NamedSignalLayout, PinError, PinnedSeed, ReportLayout, SeedLayout, SeedSourceFacts, SlotLayout,
    SourceSeedId, SEED_LAYOUT_FORMAT,
};
pub use types::{Evidence, Session, Source};

pub(crate) fn sha(s: impl AsRef<[u8]>) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(s.as_ref())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
