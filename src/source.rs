//! Exact source bytes and their SHA-256 capture identity.
//!
//! The bytes are the capture identity input: they are hashed before any UTF-8
//! decoding or normalization takes place.

use std::{fs, io, path::Path};

use sha2::{Digest, Sha256};

/// Source bytes and their UTF-8 text. The bytes are the capture identity input;
/// they are hashed before any decoding or normalization takes place.
pub struct SourceBytes {
    pub bytes: Vec<u8>,
    pub text: String,
}

/// Read one source file exactly once. Hashing callers use [`sha256_hex`] on
/// `bytes` so capture identity is independent of UTF-8 conversion.
pub fn read_source(path: impl AsRef<Path>) -> io::Result<SourceBytes> {
    let bytes = fs::read(path)?;
    let text = String::from_utf8(bytes.clone())
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    Ok(SourceBytes { bytes, text })
}

/// Lowercase hexadecimal SHA-256 of the supplied bytes.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}
