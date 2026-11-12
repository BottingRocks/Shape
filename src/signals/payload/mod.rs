//! Generic report payloads and caller-produced opaque bytes.
//!
//! The codecs perform no signal collection or signal-layout interpretation.
//! [`extract`] analyzes supported clean emitted programs to supply configuration. [`Profile`] supplies all cipher material; [`Payload`] supplies
//! the two seeds and either plaintext bytes or an opaque [`Container`]. Decode
//! returns raw bytes in the same [`Payload`] format accepted by encode.
//! [`PayloadCodec`] also converts [`report::Report`] directly to/from finalized
//! payload text, packing generic values without any header handling.
//!
//! Payloads are **unauthenticated**. Successful decryption says nothing about
//! the key or the meaning of the bytes. Explicit container parsing checks only
//! framing, never signal semantics or authenticity.
//!
//! # One seed, one artifact, one codec
//!
//! A captured seed is pinned once into a [`SeedLayout`] artifact (`shape gen
//! pin` will live in the root `shape` CLI). The artifact records the cipher
//! geometry, the header policy, the session literals, the ordered omitted
//! keysets and the report layout: the four source channels, with one entry per
//! positional slot and its descriptor. Loading it is
//! [`SeedLayout::from_json`] plus validation; there is no source parse.
//!
//! The wire path is the same in both directions and includes every layer:
//!
//! ```text
//! encode: channels -> superpack -> container (ULEB + sentinel) -> seed prefix
//!         -> ChaCha-style cipher -> unpadded custom base64 -> header chunks,
//!         b checksum, f/c/d/z siblings
//! decode: the same stages reversed; positional `extra` bytes stay opaque
//! ```
//!
//! [`SeedLayout::encode_payload`] and [`SeedLayout::encode_headers`] run that
//! path, and [`SeedLayout::decode_payload`] / [`SeedLayout::decode_headers`]
//! reverse it with the artifact's own count and keysets. [`ChannelReport`] is
//! the owned four-channel document shared by both directions; signals are raw
//! input/output here, never collected or interpreted.

pub mod extract;
pub use extract::{
    pin, session_from_initializer, PinError, PinnedSeed, SeedLayout, SEED_LAYOUT_FORMAT,
};
pub use report::{ChannelReport, Report};

mod headers;
pub mod report;
pub mod superpack;
pub mod typed_json;
pub use headers::{header_checksum, HeaderContext, HeaderReport, Headers};

use serde::{Deserialize, Serialize};

use crate::signals::codec::{
    base64_decode, base64_encode, crypt, pack_container, uleb, unpack_container_bounded, Build,
    CodecError,
};
pub use crate::signals::codec::{Container, KeyedEntry, PositionalEntry};

/// The profile version fixes the cipher, modulo-2^53 counter progression,
/// big-endian seed prefix, and canonical unpadded custom-base64 policy.
pub const PROFILE_FORMAT: &str = "shape-byte-codec/1";
/// Maximum plaintext size (16 MiB), before encryption or container packing.
pub const MAX_PLAINTEXT_BYTES: usize = 16 * 1024 * 1024;
/// Combined positional and keyed record limit.
pub const MAX_CONTAINER_ENTRIES: usize = 65_536;
/// Maximum canonical payload string size, including its eight-byte prefix.
pub const MAX_HEADER_BYTES: usize = ((MAX_PLAINTEXT_BYTES + 8) * 4).div_ceil(3);

/// Explicit geometry. `double_rounds: 10` runs 20 ChaCha rounds, with this
/// format's 64-bit counter/64-bit seed state rather than an IETF nonce layout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CipherProfile {
    pub ivs: [u32; 4],
    /// Initial counter; must be less than 2^53. Advances modulo 2^53.
    pub counter: u64,
    /// Required, in 1..=20; captured vectors cover 4, 5, and 6.
    pub double_rounds: u32,
}

/// Payload-only cipher configuration. No header names, tokens or context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PayloadProfile {
    pub format: String,
    pub cipher: CipherProfile,
    pub alphabet: String,
    pub keys: [u32; 8],
}

impl From<Profile> for PayloadProfile {
    fn from(profile: Profile) -> Self {
        Self {
            format: profile.format,
            cipher: profile.cipher,
            alphabet: profile.alphabet,
            keys: profile.keys,
        }
    }
}

/// Validated payload-only byte codec, shared by report and header APIs.
#[derive(Clone)]
pub struct PayloadCodec {
    profile: PayloadProfile,
    build: Build,
}

/// Caller-supplied cipher material and one `-a` header name. There are no
/// captured constants or inferred facts. Header formatting may optionally use
/// a supplied transaction token; it does not enter the byte cipher.
/// The first 64 alphabet bytes are data symbols (including `=` if present).
/// An optional 65th unused symbol accommodates launcher alphabets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub format: String,
    pub cipher: CipherProfile,
    pub alphabet: String,
    pub keys: [u32; 8],
    pub header_name: String,
    /// Required only when emitting/checking the b checksum or f token header.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transaction_token: Option<String>,
    /// Explicit maximum characters per a chunk. None means one unsplit header.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chunk_size: Option<usize>,
    /// Optional caller-supplied sibling policy/context. If also supplied in a
    /// payload, both contexts must agree exactly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header_context: Option<HeaderContext>,
    /// Source-derived positional slot count. When present, decode commands use
    /// it instead of requiring `--positional-count`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub positional_count: Option<usize>,
    /// Source-derived ordered omitted keysets. When present, decode commands
    /// use them instead of requiring `--keysets`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keysets: Option<Vec<Vec<String>>>,
}

/// Mutually exclusive plaintext representations. All bytes are opaque. A
/// positional `null` is the two-zero-byte sentinel. Keyed records retain order,
/// including repeated numeric names. No signal seeds are added to any section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Data {
    Raw { bytes: Vec<u8> },
    Container { container: Container },
}

/// Encode input and decode output, with required per-payload seeds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Payload {
    pub seeds: [u32; 2],
    pub data: Data,
    /// Preserved sibling values/presence, without retaining encrypted a chunks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header_context: Option<HeaderContext>,
}

impl Payload {
    pub fn raw(seeds: [u32; 2], bytes: Vec<u8>) -> Self {
        Self {
            seeds,
            data: Data::Raw { bytes },
            header_context: None,
        }
    }

    /// Return plaintext without interpreting opaque section contents.
    pub fn plaintext(&self) -> Result<Vec<u8>, Error> {
        match &self.data {
            Data::Raw { bytes } => {
                check_size(bytes.len())?;
                Ok(bytes.clone())
            }
            Data::Container { container } => {
                validate_container(container)?;
                Ok(pack_container(container))
            }
        }
    }

    /// Parse framing using a required positional count (not stored on wire).
    /// Reject noncanonical framing rather than losing its original bytes.
    /// Keep/use the raw payload when arbitrary or noncanonical framing is needed.
    pub fn parse_container(&self, positional_count: usize) -> Result<Self, Error> {
        let bytes = self.plaintext()?;
        let container = unpack_container_bounded(&bytes, positional_count, MAX_CONTAINER_ENTRIES)?;
        if pack_container(&container) != bytes {
            return Err(invalid(
                "noncanonical container framing; use raw bytes to preserve it",
            ));
        }
        Ok(Self {
            seeds: self.seeds,
            data: Data::Container { container },
            header_context: self.header_context.clone(),
        })
    }
}

/// A validated codec. Construct with [`Generator::new`]; profiles cannot be
/// mutated after validation. Library methods also accept/return raw values.
#[derive(Clone)]
pub struct Generator {
    profile: Profile,
    codec: PayloadCodec,
}

impl Generator {
    pub fn new(profile: Profile) -> Result<Self, Error> {
        let codec = PayloadCodec::new(profile.clone().into())?;
        validate_header_name(&profile.header_name)?;
        if !profile.header_name.to_ascii_lowercase().ends_with("-a") {
            return Err(invalid("only a caller-configured -a header is supported"));
        }
        headers::validate_profile(&profile)?;
        Ok(Self { profile, codec })
    }

    pub fn profile(&self) -> &Profile {
        &self.profile
    }

    /// Encrypt exactly these bytes, without adding container framing.
    pub fn encode_bytes(&self, plaintext: &[u8], seeds: [u32; 2]) -> Result<String, Error> {
        self.codec.encode_bytes(plaintext, seeds)
    }

    /// Encode the four-channel document with caller-supplied seeds and keysets.
    pub fn encode_channels(
        &self,
        channels: &report::ChannelReport,
        seeds: [u32; 2],
        keysets: Vec<Vec<String>>,
    ) -> Result<String, Error> {
        self.codec.encode_report(&channels.report(seeds, keysets))
    }

    pub fn encode(&self, payload: &Payload) -> Result<String, Error> {
        // A Payload can carry context even when the caller requests only the
        // joined value. Refuse conflicting/invalid context rather than ignoring it.
        self.context(payload.header_context.as_ref())?;
        self.encode_bytes(&payload.plaintext()?, payload.seeds)
    }

    /// Canonical input only: no whitespace, external padding, or nonzero tail
    /// bits. `=` remains data when in the first 64 symbols. This strict boundary
    /// leaves the legacy low-level decoder's normalization behavior unchanged.
    pub fn decode(&self, value: &str) -> Result<Payload, Error> {
        self.codec.decode(value)
    }
}

impl PayloadCodec {
    pub fn new(profile: PayloadProfile) -> Result<Self, Error> {
        if profile.format != PROFILE_FORMAT {
            return Err(invalid(
                "unsupported profile format; expected shape-byte-codec/1",
            ));
        }
        if !(1..=20).contains(&profile.cipher.double_rounds) {
            return Err(invalid(
                "cipher.double_rounds must be in 1..=20 (double rounds)",
            ));
        }
        if profile.cipher.counter >= 1u64 << 53 {
            return Err(invalid("cipher.counter must be less than 2^53"));
        }
        let alphabet = profile.alphabet.as_bytes();
        if !(64..=65).contains(&alphabet.len())
            || !alphabet.iter().all(|b| (0x21..=0x7e).contains(b))
        {
            return Err(invalid(
                "alphabet needs 64 or 65 visible ASCII symbols without whitespace",
            ));
        }
        let mut seen = [false; 128];
        for &byte in alphabet {
            if std::mem::replace(&mut seen[byte as usize], true) {
                return Err(invalid(
                    "alphabet symbols must be unique (including the unused 65th symbol)",
                ));
            }
        }
        let build = Build {
            ivs: profile.cipher.ivs,
            counter: profile.cipher.counter,
            rounds: profile.cipher.double_rounds,
        };
        Ok(Self { profile, build })
    }

    /// Encrypt bytes with the explicit seeds and add the eight-byte seed prefix.
    pub fn encode_bytes(&self, plaintext: &[u8], seeds: [u32; 2]) -> Result<String, Error> {
        check_size(plaintext.len())?;
        let mut raw = Vec::with_capacity(plaintext.len() + 8);
        raw.extend_from_slice(&seeds[0].to_be_bytes());
        raw.extend_from_slice(&seeds[1].to_be_bytes());
        raw.extend(crypt(plaintext, &self.profile.keys, seeds, &self.build));
        Ok(base64_encode(&raw, &self.profile.alphabet)?)
    }

    /// Decode canonical custom base64 into raw plaintext and recovered seeds.
    /// No header context, positional-count inference or value parsing occurs.
    pub fn decode(&self, value: &str) -> Result<Payload, Error> {
        let raw = self.validate_finalized_payload(value)?;
        let seeds = [
            u32::from_be_bytes(raw[0..4].try_into().expect("checked prefix")),
            u32::from_be_bytes(raw[4..8].try_into().expect("checked prefix")),
        ];
        Ok(Payload::raw(
            seeds,
            crypt(&raw[8..], &self.profile.keys, seeds, &self.build),
        ))
    }

    // Validate wire text without decrypting or encrypting a finalized payload.
    fn validate_finalized_payload(&self, value: &str) -> Result<Vec<u8>, Error> {
        if value.len() > MAX_HEADER_BYTES {
            return Err(invalid("payload value exceeds the size limit"));
        }
        let raw = base64_decode(value, &self.profile.alphabet)?;
        if base64_encode(&raw, &self.profile.alphabet)? != value {
            return Err(invalid(
                "payload value must be canonical unpadded custom base64",
            ));
        }
        if raw.len() < 8 {
            return Err(CodecError::ShortPayload(raw.len()).into());
        }
        check_size(raw.len() - 8)?;
        Ok(raw)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Invalid(String),
    #[error(transparent)]
    Codec(#[from] CodecError),
}

fn invalid(message: &str) -> Error {
    Error::Invalid(message.to_owned())
}

fn check_size(size: usize) -> Result<(), Error> {
    if size > MAX_PLAINTEXT_BYTES {
        return Err(invalid("plaintext exceeds the 16 MiB limit"));
    }
    Ok(())
}

fn validate_header_name(name: &str) -> Result<(), Error> {
    if name.is_empty()
        || name.len() > 256
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
    {
        return Err(invalid(
            "header name must be an HTTP token of 1..=256 ASCII bytes",
        ));
    }
    Ok(())
}

fn validate_container(container: &Container) -> Result<(), Error> {
    if container
        .positional
        .len()
        .saturating_add(container.keyed.len())
        > MAX_CONTAINER_ENTRIES
    {
        return Err(CodecError::TooManyEntries.into());
    }
    // Compute/check the packed size before allocating an output buffer. All
    // additions stop at the small byte limit, so cannot overflow usize.
    let mut size = 0;
    let mut add = |n: usize| -> Result<(), Error> {
        check_size(n)?;
        size += n;
        check_size(size)
    };
    let mut blob = |bytes: &[u8]| -> Result<(), Error> {
        add(uleb(bytes.len() as u64).len())?;
        add(bytes.len())
    };
    blob(&container.errors)?;
    blob(&container.signals)?;
    for entry in container.positional.iter().flatten() {
        if entry.encoded.is_empty() && entry.extra.is_empty() {
            return Err(invalid(
                "an empty positional entry is ambiguous; use null for the sentinel",
            ));
        }
        blob(&entry.encoded)?;
        blob(&entry.extra)?;
    }
    for entry in &container.keyed {
        blob(&entry.encoded)?;
        blob(&entry.extra)?;
    }
    for _ in container.positional.iter().filter(|entry| entry.is_none()) {
        add(2)?;
    }
    for entry in &container.keyed {
        add(uleb(u64::from(entry.name)).len())?;
    }
    Ok(())
}
