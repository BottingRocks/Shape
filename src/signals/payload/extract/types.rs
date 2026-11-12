//! Provenance and session material shared by seed pinning.
use serde::{Deserialize, Serialize};

/// Digest and length of the traced program a layout was pinned from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub sha256: String,
    pub bytes: usize,
}

/// The byte span and digest of one pinned fact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    pub role: String,
    pub start: u32,
    pub end: u32,
    pub sha256: String,
}

/// The launcher's session literals for one capture.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Session {
    pub transaction_token: String,
    pub alphabet: String,
    pub shuffle_key: Vec<u32>,
    pub keys: [u32; 8],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nonce: Option<String>,
}
