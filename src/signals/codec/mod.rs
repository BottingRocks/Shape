//! Wire codec for the positional signals of a Shape-style capture.
//!
//! This is the Rust port of the format implemented by
//! `reports/full-naming-samples/target-seed-1.latest.renamed.js`. Every
//! constant and byte layout below is taken from that file; the port is checked
//! byte-for-byte against it (see the known-answer tests at the bottom and the
//! JavaScript oracle in `reports/full-naming-samples/signal-codec/`).
//!
//! # The three writers
//!
//! A positional signal is `[be32 seed][body]`. `body` is produced by three
//! functions in the capture, plus inline flag bytes:
//!
//! | writer | capture line | layout |
//! |---|---:|---|
//! | keystream `createPrngByteSource` | 1054 | one mask byte per encoded byte |
//! | string `createStringInternWriter` | 898 | `129` + table index, or modified UTF-8 + `0` |
//! | number `$THREAD_7_24052` | 957 | sign/magnitude varint, or `128` + IEEE-754 LE |
//!
//! # Keystream
//!
//! The generator state is `s2 = hash_string_token(transaction_token)` and
//! `s5 = seed ^ C`, where `C` is a static per-signal build constant. One
//! 4-byte little-endian block is produced per four calls:
//!
//! ```text
//! out = s2 + s5                    (mod 2^32, little-endian bytes)
//! t   = s5 ^ s2
//! s2  = t ^ rotl(s2, 26) ^ (t << 9)
//! s5  = rotl(t, 13)
//! ```
//!
//! The factory burns one whole block before the first write, so a decoder must
//! burn it too.
//!
//! # Flow to the encryption
//!
//! ```text
//! signal bodies
//!   -> container: errors || positional (encoded, extra) || signals || keyed
//!      with unsigned-LEB128 lengths and a two-zero-byte sentinel
//!   -> stream cipher: 16 x u32 state, `Build::rounds` double rounds,
//!      add-back, little-endian pack, XOR (see `Build::TARGET_SEED_1`)
//!   -> payload: base64(alphabet, be32(seed1) || be32(seed2) || ciphertext)
//! ```
//!
//! # Sample data
//!
//! [`layout`] puts the write program in data instead of code, so arbitrary
//! sample values can be encoded and decoded today:
//!
//! ```json
//! {"kind": "number", "name": "status"}
//! ```
//!
//! A layout is a list of such fields; the matching value is a JSON object
//! keyed by field name. The real per-signal mappings can later be emitted as
//! the same layout data without touching this module.
//!
//! # Example
//!
//! The session values below are the launcher's
//! `init(token, alphabet, [], keys, nonce, nonce, [], a, src)` arguments:
//!
//! ```
//! use shape::signals::codec::{decode_signal, encode_signal, Session};
//!
//! let session = Session::new(
//!     "A4peOJGhAQAA_R6jhFOmE96X3pjkjUK24DtgrMqnzp4dlOASRMAInfR-00XoAUm92wCuclLXwH9eCOfvosJeCA==",
//!     "768eqNwP4xRQJ0kacboBKn5j1ugh9SIiVvZsmrtMzF=_OlWD2UGp3AHE-dCLYTXyf",
//!     [
//!         1944786431, 1181771095, 2061591115, 263772366, 1195508087, 1094872937, 60797880,
//!         642840060,
//!     ],
//! );
//!
//! // one signal: seed 0x11223344 and the collector's static XOR for this signal
//! let xor = 2_363_885_845;
//! let extra = encode_signal(session.token_hash, 0x1122_3344, xor, |w| {
//!     w.number(3.0);
//!     w.byte(0);
//!     w.string("ok");
//! });
//! assert_eq!(&extra[..4], &[0x11, 0x22, 0x33, 0x44]);
//!
//! let (value, seed) = decode_signal(&extra, session.token_hash, xor, |r| {
//!     Ok((r.number()?, r.byte()?, r.string()?))
//! })?;
//! assert_eq!(seed, 0x1122_3344);
//! assert_eq!(value, (3.0, 0, "ok".to_owned()));
//! # Ok::<(), shape::signals::codec::CodecError>(())
//! ```

pub mod layout;

pub use layout::{decode_layout, encode_layout, Field, Layout, SignalValue};

use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};

// ===========================================================================
// build constants (literals in the target file)
// ===========================================================================

/// Cipher state `[0..3]` (target file line 3294).
pub const TARGET_SEED_1_IVS: [u32; 4] = [522_575_410, 361_402_133, 1_980_070_771, 3_031_302_865];

/// Initial 64-bit block counter (target file line 3312).
pub const TARGET_SEED_1_COUNTER: u64 = 307_069_178_630_199;

/// Double rounds per 64-byte block: 40 unrolled quarter-rounds (lines 3318-3795).
pub const TARGET_SEED_1_ROUNDS: u32 = 5;

/// Static cipher geometry of one capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Build {
    /// `state[0..3]`.
    pub ivs: [u32; 4],
    /// `state[12..13]` as one 64-bit counter.
    pub counter: u64,
    /// Double rounds per 64-byte block.
    pub rounds: u32,
}

impl Build {
    /// `target-seed-1.latest.renamed.js`.
    pub const TARGET_SEED_1: Self = Self {
        ivs: TARGET_SEED_1_IVS,
        counter: TARGET_SEED_1_COUNTER,
        rounds: TARGET_SEED_1_ROUNDS,
    };
}

impl Default for Build {
    fn default() -> Self {
        Self::TARGET_SEED_1
    }
}

// ===========================================================================
// session
// ===========================================================================

/// The launcher `init(...)` values for one session.
///
/// `token` is hashed into the per-signal keystream; `alphabet` and `keys` are
/// used by the payload cipher. `nonce` is not part of the wire format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    /// `init` argument 1 (transaction token).
    pub token: String,
    /// `hash_string_token(&token)`, cached.
    pub token_hash: i32,
    /// `init` argument 2 (at least 64 symbols; `=` may be a data symbol).
    pub alphabet: String,
    /// `init` argument 4.
    pub keys: [u32; 8],
}

impl Session {
    /// Hash the token and store the session.
    pub fn new(token: impl Into<String>, alphabet: impl Into<String>, keys: [u32; 8]) -> Self {
        let token = token.into();
        let token_hash = hash_string_token(&token);
        Self {
            token,
            token_hash,
            alphabet: alphabet.into(),
            keys,
        }
    }

    /// Use a precomputed token hash (when the token itself is not retained).
    pub fn with_token_hash(token_hash: i32, alphabet: impl Into<String>, keys: [u32; 8]) -> Self {
        Self {
            token: String::new(),
            token_hash,
            alphabet: alphabet.into(),
            keys,
        }
    }
}

/// `hashStringOrBuffer` for a string (target file line 1250): the Java-style
/// `h * 31` hash over UTF-16 code units, in pairs.
pub fn hash_string_token(value: &str) -> i32 {
    let units: Vec<u16> = value.encode_utf16().collect();
    let even = units.len() & !1;
    let mut hash: i32 = 0;
    let mut i = 0;
    while i < even {
        let pair = ((units[i] as i32) << 16) | units[i + 1] as i32;
        hash = hash.wrapping_mul(31).wrapping_add(pair);
        i += 2;
    }
    if even != units.len() {
        let last = (units[even] as i32) << 16;
        hash = hash.wrapping_mul(31).wrapping_add(last);
    }
    hash
}

// ===========================================================================
// errors
// ===========================================================================

/// A malformed body, container, or payload.
#[derive(Debug, Clone, PartialEq)]
pub enum CodecError {
    /// A signal body must open with its 4-byte big-endian seed.
    ShortSeedPrefix(usize),
    /// A read ran past the end of its buffer.
    Truncated(&'static str),
    /// The number writer's float marker is not `0x80`.
    BadFloatMarker(u8),
    /// A string's modified-UTF-8 lead byte is invalid.
    BadUtf8(u8),
    /// A string-table back-reference is out of range.
    StringTableRef(u32),
    /// A character is not in the session alphabet.
    BadBase64(char),
    /// The alphabet needs 64 symbols.
    ShortAlphabet(usize),
    /// The container ended before every section was read.
    BlobOverrun,
    /// Parsing did not consume the whole container.
    TrailingBytes(usize),
    /// No single positional count consumes the container exactly.
    PositionalCount(Vec<usize>),
    /// The payload is shorter than its 8-byte seed prefix.
    ShortPayload(usize),
    /// A base64 length leaves a dangling symbol.
    BadBase64Length(usize),
    /// A read program stopped before the end of the body.
    UnreadBytes(usize),
    /// A string-table back-reference is not a table index.
    BadStringIndex(f64),
    /// A layout field has no value.
    MissingField(String),
    /// A layout value does not have the type the field needs.
    WrongValue {
        /// Field name (or `<root>`).
        field: String,
        /// What the field accepts.
        expected: &'static str,
    },
    /// A `byte` field is outside `0..=255`.
    ByteOutOfRange {
        /// Field name.
        field: String,
        /// Supplied value.
        value: f64,
    },
    /// An array count is negative, fractional, or larger than the body.
    ArrayCount {
        /// Field name.
        field: String,
        /// Supplied or decoded count.
        count: f64,
    },
    /// An unsigned LEB128 integer exceeds 64 bits.
    UlebOverflow,
    /// A keyed name exceeds the format's 32-bit width.
    KeyedNameOutOfRange(u64),
    /// A caller's container record budget was exceeded.
    TooManyEntries,
}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ShortSeedPrefix(n) => {
                write!(
                    f,
                    "signal body needs its 4-byte seed prefix (got {n} bytes)"
                )
            }
            Self::Truncated(what) => write!(f, "truncated {what}"),
            Self::BadFloatMarker(b) => write!(f, "bad float marker 0x{b:02x}"),
            Self::BadUtf8(b) => write!(f, "bad modified-UTF-8 lead byte 0x{b:02x}"),
            Self::StringTableRef(index) => {
                write!(f, "string table reference {index} out of range")
            }
            Self::BadBase64(c) => write!(f, "character not in alphabet: {c:?}"),
            Self::ShortAlphabet(n) => {
                write!(f, "alphabet needs at least 64 characters (got {n})")
            }
            Self::BlobOverrun => write!(f, "blob overruns container"),
            Self::TrailingBytes(n) => write!(f, "container has {n} trailing bytes"),
            Self::PositionalCount(candidates) => {
                write!(
                    f,
                    "no single positional count consumes the container: {candidates:?}"
                )
            }
            Self::ShortPayload(n) => {
                write!(f, "payload needs the 8-byte seed prefix (got {n} bytes)")
            }
            Self::BadBase64Length(n) => {
                write!(f, "base64 has {n} symbols, which leaves a dangling symbol")
            }
            Self::UnreadBytes(n) => write!(f, "signal body has {n} unread bytes"),
            Self::BadStringIndex(index) => {
                write!(f, "string table reference {index} is not a valid index")
            }
            Self::MissingField(field) => write!(f, "layout field {field:?} has no value"),
            Self::WrongValue { field, expected } => {
                write!(f, "layout field {field:?} needs {expected}")
            }
            Self::ByteOutOfRange { field, value } => {
                write!(f, "layout field {field:?} is outside 0..=255: {value}")
            }
            Self::ArrayCount { field, count } => {
                write!(
                    f,
                    "layout field {field:?} has an invalid array count: {count}"
                )
            }
            Self::UlebOverflow => write!(f, "uleb128 exceeds 64 bits"),
            Self::KeyedNameOutOfRange(name) => write!(f, "keyed name exceeds u32: {name}"),
            Self::TooManyEntries => write!(f, "container exceeds the entry limit"),
        }
    }
}

impl std::error::Error for CodecError {}

// ===========================================================================
// keystream
// ===========================================================================

/// `createPrngByteSource` (target file line 1054).
#[derive(Debug, Clone)]
pub struct Keystream {
    s2: u32,
    s5: u32,
    block: u32,
    pos: u8,
}

impl Keystream {
    /// `token_hash` is `s2`; `seed_value` is `seed ^ C`.
    pub fn new(token_hash: i32, seed_value: u32) -> Self {
        let mut stream = Self {
            s2: token_hash as u32,
            s5: seed_value,
            block: 0,
            pos: 4,
        };
        // The factory burns one whole block before the first write.
        stream.next_byte();
        stream.next_byte();
        stream.next_byte();
        stream.next_byte();
        stream
    }

    fn next_block(&mut self) {
        let s2 = self.s2;
        let s5 = self.s5;
        let out = s2.wrapping_add(s5);
        let t = s5 ^ s2;
        self.s2 = t ^ s2.rotate_left(26) ^ (t << 9);
        self.s5 = t.rotate_left(13);
        self.block = out;
    }

    /// Next mask byte.
    pub fn next_byte(&mut self) -> u8 {
        if self.pos >= 4 {
            self.next_block();
            self.pos = 0;
        }
        let byte = (self.block >> (8 * self.pos)) as u8;
        self.pos += 1;
        byte
    }
}

// ===========================================================================
// writer
// ===========================================================================

/// Signal-body writer: the Rust form of the collector's write sequence.
#[derive(Debug, Clone)]
pub struct SignalWriter {
    keystream: Keystream,
    mask_enabled: bool,
    bytes: Vec<u8>,
    index: usize,
    strings: HashMap<Vec<u16>, usize>,
    next_string: usize,
}

impl SignalWriter {
    /// Start a body. `seed` becomes the plaintext `be32` prefix; `xor` is the
    /// collector's static constant `C`.
    pub fn new(token_hash: i32, seed: u32, xor: u32) -> Self {
        let keystream = Keystream::new(token_hash, seed ^ xor);
        let bytes = seed.to_be_bytes().to_vec();
        Self {
            keystream,
            mask_enabled: true,
            bytes,
            index: 3,
            strings: HashMap::new(),
            next_string: 0,
        }
    }

    /// One plaintext byte (flag, packed bitfield, or branch tag).
    pub fn byte(&mut self, value: u8) {
        self.masked(value);
    }

    /// Number writer `$THREAD_7_24052` (target file line 957).
    pub fn number(&mut self, value: f64) {
        self.number_with_float_order(value, false);
    }

    /// Explicit IEEE byte order; `big_endian = false` selects little endian.
    /// Legacy `number` remains little endian.
    pub fn number_with_float_order(&mut self, value: f64, big_endian: bool) {
        match int32(value) {
            Some(int) => {
                // `-0.0` is an int32 too; keep the sign the cast would lose.
                let negative = value < 0.0 || (value == 0.0 && value.is_sign_negative());
                self.integer(int, negative);
            }
            None => self.float_with_order(value, big_endian),
        }
    }

    fn integer(&mut self, value: i32, negative: bool) {
        let (code, magnitude) = if negative {
            (64u8, value.unsigned_abs())
        } else {
            (0u8, value as u32)
        };
        let m = magnitude;
        if m < 32 {
            self.masked(code | m as u8);
        } else if m < 4096 {
            self.masked(code | ((m as u8) & 31) | 32);
            self.masked((m >> 5) as u8);
        } else if m < 524_288 {
            self.masked(code | ((m as u8) & 31) | 32);
            self.masked(((m >> 5) & 127) as u8 | 128);
            self.masked((m >> 12) as u8);
        } else if m < 67_108_864 {
            self.masked(code | ((m as u8) & 31) | 32);
            self.masked(((m >> 5) & 127) as u8 | 128);
            self.masked(((m >> 12) & 127) as u8 | 128);
            self.masked((m >> 19) as u8);
        } else {
            self.masked(code | ((m as u8) & 31) | 32);
            self.masked(((m >> 5) & 127) as u8 | 128);
            self.masked(((m >> 12) & 127) as u8 | 128);
            self.masked(((m >> 19) & 127) as u8 | 128);
            self.masked((m >> 26) as u8);
        }
    }

    fn float_with_order(&mut self, value: f64, big_endian: bool) {
        // Marker 128, then eight IEEE backing bytes in the selected order.
        self.masked(128);
        for byte in if big_endian {
            value.to_be_bytes()
        } else {
            value.to_le_bytes()
        } {
            self.masked(byte);
        }
    }

    /// Interned string writer (target file line 898).
    pub fn string(&mut self, value: &str) {
        self.string_units(&value.encode_utf16().collect::<Vec<_>>());
    }

    /// The source writer operates on UTF-16 units, including lone surrogates.
    pub fn string_units(&mut self, units: &[u16]) {
        if let Some(&index) = self.strings.get(units) {
            self.masked(129);
            self.integer(index as i32, false);
            return;
        }
        let mut i = 0;
        while i < units.len() {
            let unit = units[i];
            if unit == 0 {
                self.masked(0xc0);
                self.masked(0x80);
            } else if unit < 0x80 {
                self.masked(unit as u8);
            } else if unit < 0x800 {
                self.masked(((unit >> 6) | 0xc0) as u8);
                self.masked(((unit & 63) | 0x80) as u8);
            } else {
                let next = if (0xd800..0xdc00).contains(&unit) && i + 1 < units.len() {
                    units[i + 1]
                } else {
                    0
                };
                if (0xdc00..0xe000).contains(&next) {
                    let cp = 0x10000u32 + (((unit as u32) - 0xd800) << 10) + (next as u32 - 0xdc00);
                    self.masked(((cp >> 18) | 0xf0) as u8);
                    self.masked((((cp >> 12) & 63) | 0x80) as u8);
                    self.masked((((cp >> 6) & 63) | 0x80) as u8);
                    self.masked(((cp & 63) | 0x80) as u8);
                    i += 1;
                } else {
                    self.masked(((unit >> 12) | 0xe0) as u8);
                    self.masked((((unit >> 6) & 63) | 0x80) as u8);
                    self.masked(((unit & 63) | 0x80) as u8);
                }
            }
            i += 1;
        }
        self.masked(0);
        self.strings.insert(units.to_vec(), self.next_string);
        self.next_string += 1;
    }

    fn masked(&mut self, byte: u8) {
        let mask = if self.mask_enabled {
            self.keystream.next_byte()
        } else {
            0
        };
        self.index += 1;
        if self.bytes.len() <= self.index {
            self.bytes.resize(self.index + 1, 0);
        }
        self.bytes[self.index] = byte ^ mask;
    }

    /// `[be32 seed][body]`.
    pub fn finish(self) -> Vec<u8> {
        self.bytes
    }

    pub(crate) fn masking(&mut self, enabled: bool) {
        self.mask_enabled = enabled;
    }
}

fn int32(value: f64) -> Option<i32> {
    if !value.is_finite() {
        return None;
    }
    let int = value as i32;
    if int as f64 == value {
        Some(int)
    } else {
        None
    }
}

// ===========================================================================
// reader
// ===========================================================================

/// Signal-body reader: the inverse of [`SignalWriter`].
#[derive(Debug, Clone)]
pub struct SignalReader<'a> {
    keystream: Keystream,
    mask_enabled: bool,
    bytes: &'a [u8],
    pos: usize,
    pending: Option<u8>,
    strings: Vec<Vec<u16>>,
}

impl<'a> SignalReader<'a> {
    /// `xor` is the collector's static constant `C`; `bytes` is `[be32 seed][body]`.
    pub fn new(bytes: &'a [u8], token_hash: i32, xor: u32) -> Result<Self, CodecError> {
        if bytes.len() < 4 {
            return Err(CodecError::ShortSeedPrefix(bytes.len()));
        }
        let seed = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        Ok(Self {
            keystream: Keystream::new(token_hash, seed ^ xor),
            mask_enabled: true,
            bytes,
            pos: 4,
            pending: None,
            strings: Vec::new(),
        })
    }

    /// The per-signal seed from the prefix.
    pub fn seed(&self) -> u32 {
        u32::from_be_bytes([self.bytes[0], self.bytes[1], self.bytes[2], self.bytes[3]])
    }

    /// One plaintext byte.
    pub fn byte(&mut self) -> Result<u8, CodecError> {
        if let Some(byte) = self.pending.take() {
            return Ok(byte);
        }
        if self.pos >= self.bytes.len() {
            return Err(CodecError::Truncated("signal body"));
        }
        let raw = self.bytes[self.pos];
        self.pos += 1;
        Ok(raw
            ^ if self.mask_enabled {
                self.keystream.next_byte()
            } else {
                0
            })
    }

    /// Consume `tag` if it is next, otherwise leave the byte for the next read.
    pub fn peek_tag(&mut self, tag: u8) -> Result<bool, CodecError> {
        let byte = self.byte()?;
        if byte == tag {
            Ok(true)
        } else {
            self.pending = Some(byte);
            Ok(false)
        }
    }

    /// Inverse of the number writer. Integer values come back exactly (as
    /// `f64`); `-0.0` keeps its sign.
    pub fn number(&mut self) -> Result<f64, CodecError> {
        self.number_with_float_order(false)
    }

    /// Explicit IEEE byte order; `big_endian = false` selects little endian.
    /// Legacy `number` remains little endian.
    pub fn number_with_float_order(&mut self, big_endian: bool) -> Result<f64, CodecError> {
        let first = self.byte()?;
        if first & 0x80 != 0 {
            if first != 0x80 {
                return Err(CodecError::BadFloatMarker(first));
            }
            let mut bytes = [0u8; 8];
            for byte in &mut bytes {
                *byte = self.byte()?;
            }
            return Ok(if big_endian {
                f64::from_be_bytes(bytes)
            } else {
                f64::from_le_bytes(bytes)
            });
        }
        let negative = first & 64 != 0;
        let mut magnitude = u32::from(first & 31);
        if first & 32 != 0 {
            let mut byte = self.byte()?;
            magnitude |= u32::from(byte & 127) << 5;
            if byte & 128 != 0 {
                byte = self.byte()?;
                magnitude |= u32::from(byte & 127) << 12;
                if byte & 128 != 0 {
                    byte = self.byte()?;
                    magnitude |= u32::from(byte & 127) << 19;
                    if byte & 128 != 0 {
                        byte = self.byte()?;
                        magnitude |= u32::from(byte) << 26;
                    }
                }
            }
        }
        let value = f64::from(magnitude);
        Ok(if negative { -value } else { value })
    }

    /// Inverse of the interned string writer.
    ///
    /// UTF-16 units that cannot form a valid `String` (a lone surrogate) are
    /// replaced with U+FFFD by [`String::from_utf16_lossy`].
    pub fn string(&mut self) -> Result<String, CodecError> {
        self.string_units()
            .map(|units| String::from_utf16_lossy(&units))
    }

    /// Lossless inverse: interning retains exact UTF-16 units.
    pub fn string_units(&mut self) -> Result<Vec<u16>, CodecError> {
        let first = self.byte()?;
        if first == 0 {
            let empty = Vec::new();
            self.strings.push(empty.clone());
            return Ok(empty);
        }
        if first == 129 {
            let index = self.number()?;
            if index < 0.0 || index.fract() != 0.0 || index > u32::MAX as f64 {
                return Err(CodecError::BadStringIndex(index));
            }
            let index = index as u32;
            return self
                .strings
                .get(index as usize)
                .cloned()
                .ok_or(CodecError::StringTableRef(index));
        }
        let mut units: Vec<u16> = Vec::new();
        let mut byte = first;
        while byte != 0 {
            if byte < 0x80 {
                units.push(u16::from(byte));
            } else if byte == 0xc0 {
                let next = self.byte()?;
                if next != 0x80 {
                    return Err(CodecError::BadUtf8(byte));
                }
                units.push(0);
            } else if byte & 0xe0 == 0xc0 && byte >= 0xc2 {
                let b2 = self.continuation()?;
                units.push((u16::from(byte & 31) << 6) | u16::from(b2 & 63));
            } else if byte & 0xf0 == 0xe0 {
                let b2 = self.continuation()?;
                let b3 = self.continuation()?;
                let unit =
                    (u16::from(byte & 15) << 12) | (u16::from(b2 & 63) << 6) | u16::from(b3 & 63);
                if unit < 0x800 {
                    return Err(CodecError::BadUtf8(byte));
                }
                units.push(unit);
            } else if byte & 0xf8 == 0xf0 {
                let b2 = self.continuation()?;
                let b3 = self.continuation()?;
                let b4 = self.continuation()?;
                let mut cp = (u32::from(byte & 7) << 18)
                    | (u32::from(b2 & 63) << 12)
                    | (u32::from(b3 & 63) << 6)
                    | u32::from(b4 & 63);
                if !(0x10000..=0x10ffff).contains(&cp) {
                    return Err(CodecError::BadUtf8(byte));
                }
                cp -= 0x10000;
                units.push(0xd800 + (cp >> 10) as u16);
                units.push(0xdc00 + (cp & 1023) as u16);
            } else {
                return Err(CodecError::BadUtf8(byte));
            }
            byte = self.byte()?;
        }
        self.strings.push(units.clone());
        Ok(units)
    }

    fn continuation(&mut self) -> Result<u8, CodecError> {
        let byte = self.byte()?;
        if byte & 0xc0 != 0x80 {
            return Err(CodecError::BadUtf8(byte));
        }
        Ok(byte)
    }

    /// True when the whole body, including the terminator, has been consumed.
    pub fn at_end(&self) -> bool {
        self.pending.is_none() && self.pos == self.bytes.len()
    }

    /// Bytes still unread (including a pending peeked byte).
    pub fn remaining(&self) -> usize {
        self.bytes.len() - self.pos + usize::from(self.pending.is_some())
    }

    pub(crate) fn masking(&mut self, enabled: bool) {
        self.mask_enabled = enabled;
    }
}

// ===========================================================================
// signal helpers
// ===========================================================================

/// Build one `[be32 seed][body]` with a write program.
pub fn encode_signal(
    token_hash: i32,
    seed: u32,
    xor: u32,
    write: impl FnOnce(&mut SignalWriter),
) -> Vec<u8> {
    let mut writer = SignalWriter::new(token_hash, seed, xor);
    write(&mut writer);
    writer.finish()
}

/// Fallible [`encode_signal`], for data-driven write programs.
pub fn try_encode_signal(
    token_hash: i32,
    seed: u32,
    xor: u32,
    write: impl FnOnce(&mut SignalWriter) -> Result<(), CodecError>,
) -> Result<Vec<u8>, CodecError> {
    let mut writer = SignalWriter::new(token_hash, seed, xor);
    write(&mut writer)?;
    Ok(writer.finish())
}

/// Decode one `[be32 seed][body]` with a read program, requiring exact
/// consumption.
pub fn decode_signal<T>(
    extra: &[u8],
    token_hash: i32,
    xor: u32,
    read: impl FnOnce(&mut SignalReader<'_>) -> Result<T, CodecError>,
) -> Result<(T, u32), CodecError> {
    let mut reader = SignalReader::new(extra, token_hash, xor)?;
    let seed = reader.seed();
    let value = read(&mut reader)?;
    if !reader.at_end() {
        return Err(CodecError::UnreadBytes(reader.remaining()));
    }
    Ok((value, seed))
}

/// First four bytes of an `extra` blob (big-endian).
pub fn signal_seed(extra: &[u8]) -> Result<u32, CodecError> {
    if extra.len() < 4 {
        return Err(CodecError::ShortSeedPrefix(extra.len()));
    }
    Ok(u32::from_be_bytes([extra[0], extra[1], extra[2], extra[3]]))
}

// ===========================================================================
// container
// ===========================================================================

/// One positional entry. Both sections are opaque to container framing.
/// Captured detectors often use empty `encoded` and `[be32 seed][body]` in `extra`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PositionalEntry {
    /// Token-encoder output for `value[0]` (opaque here).
    pub encoded: Vec<u8>,
    /// Raw detector bytes, `value[1]`.
    pub extra: Vec<u8>,
}

/// One keyed signal.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyedEntry {
    /// Numeric signal name.
    pub name: u32,
    /// Token-encoder output (opaque here).
    pub encoded: Vec<u8>,
    /// Raw detector bytes.
    pub extra: Vec<u8>,
}

/// The outer container, before encryption.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Container {
    /// Token-encoder output for the error ring (opaque here).
    pub errors: Vec<u8>,
    /// `None` is the assembler's `00 00` sentinel.
    pub positional: Vec<Option<PositionalEntry>>,
    /// Token-encoder output for the separate signals blob (opaque here).
    pub signals: Vec<u8>,
    /// Keyed signals, in keyset order.
    pub keyed: Vec<KeyedEntry>,
}

/// Unsigned LEB128.
pub fn uleb(mut value: u64) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);
            return out;
        }
        out.push(byte | 0x80);
    }
}

fn read_uleb(bytes: &[u8], offset: usize) -> Result<(u64, usize), CodecError> {
    let mut value = 0u64;
    let mut shift = 0u32;
    let mut pos = offset;
    loop {
        if pos >= bytes.len() {
            return Err(CodecError::Truncated("uleb128"));
        }
        let byte = bytes[pos];
        pos += 1;
        if shift == 63 && byte > 1 {
            return Err(CodecError::UlebOverflow);
        }
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Ok((value, pos));
        }
        shift += 7;
        if shift > 63 {
            return Err(CodecError::UlebOverflow);
        }
    }
}

fn read_blob(bytes: &[u8], offset: usize) -> Result<(&[u8], usize), CodecError> {
    let (length, next) = read_uleb(bytes, offset)?;
    let length = usize::try_from(length).map_err(|_| CodecError::BlobOverrun)?;
    let end = next.checked_add(length).ok_or(CodecError::BlobOverrun)?;
    if end > bytes.len() {
        return Err(CodecError::BlobOverrun);
    }
    Ok((&bytes[next..end], end))
}

/// Serialize the outer container (target file line 14611).
pub fn pack_container(container: &Container) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend(uleb(container.errors.len() as u64));
    out.extend_from_slice(&container.errors);
    for entry in &container.positional {
        match entry {
            None => out.extend_from_slice(&[0, 0]),
            Some(entry) => {
                out.extend(uleb(entry.encoded.len() as u64));
                out.extend_from_slice(&entry.encoded);
                out.extend(uleb(entry.extra.len() as u64));
                out.extend_from_slice(&entry.extra);
            }
        }
    }
    out.extend(uleb(container.signals.len() as u64));
    out.extend_from_slice(&container.signals);
    for entry in &container.keyed {
        out.extend(uleb(u64::from(entry.name)));
        out.extend(uleb(entry.encoded.len() as u64));
        out.extend_from_slice(&entry.encoded);
        out.extend(uleb(entry.extra.len() as u64));
        out.extend_from_slice(&entry.extra);
    }
    out
}

fn parse_positional(
    bytes: &[u8],
    offset: usize,
    count: usize,
) -> Result<(Vec<Option<PositionalEntry>>, usize), CodecError> {
    // Even empty entries consume two bytes. Check before reserving capacity.
    if count > bytes.len().saturating_sub(offset) / 2 {
        return Err(CodecError::BlobOverrun);
    }
    let mut positional = Vec::with_capacity(count);
    let mut pos = offset;
    for _ in 0..count {
        let (encoded, next) = read_blob(bytes, pos)?;
        let (extra, next) = read_blob(bytes, next)?;
        pos = next;
        if encoded.is_empty() && extra.is_empty() {
            positional.push(None);
        } else {
            positional.push(Some(PositionalEntry {
                encoded: encoded.to_vec(),
                extra: extra.to_vec(),
            }));
        }
    }
    Ok((positional, pos))
}

fn parse_keyed(
    bytes: &[u8],
    offset: usize,
    limit: usize,
) -> Result<(Vec<KeyedEntry>, usize), CodecError> {
    let mut keyed = Vec::new();
    let mut pos = offset;
    while pos < bytes.len() {
        if keyed.len() >= limit {
            return Err(CodecError::TooManyEntries);
        }
        let (name, next) = read_uleb(bytes, pos)?;
        let name = u32::try_from(name).map_err(|_| CodecError::KeyedNameOutOfRange(name))?;
        let (encoded, next) = read_blob(bytes, next)?;
        let (extra, next) = read_blob(bytes, next)?;
        keyed.push(KeyedEntry {
            name,
            encoded: encoded.to_vec(),
            extra: extra.to_vec(),
        });
        pos = next;
    }
    Ok((keyed, pos))
}

/// Split a decrypted container.
///
/// The positional count is not stored; when `count` is `None` the unique count
/// that consumes the buffer exactly is used.
pub fn unpack_container(bytes: &[u8], count: Option<usize>) -> Result<Container, CodecError> {
    // Preserve the immediate framing error for an invalid errors section,
    // including when the legacy caller requests count inference.
    read_blob(bytes, 0)?;
    let attempt = |count| unpack_container_bounded(bytes, count, usize::MAX);

    if let Some(count) = count {
        return attempt(count);
    }
    let mut matches = Vec::new();
    for candidate in 0..=bytes.len() / 2 + 1 {
        if let Ok(container) = attempt(candidate) {
            matches.push((candidate, container));
        }
    }
    match matches.len() {
        1 => Ok(matches.pop().expect("one match").1),
        0 => Err(CodecError::PositionalCount(Vec::new())),
        _ => Err(CodecError::PositionalCount(
            matches.into_iter().map(|(count, _)| count).collect(),
        )),
    }
}

/// Split a container with an explicit positional count and a combined
/// positional/keyed record budget. No count inference or count-sized allocation
/// is performed before checking that the input can contain those records.
pub fn unpack_container_bounded(
    bytes: &[u8],
    count: usize,
    max_entries: usize,
) -> Result<Container, CodecError> {
    if count > max_entries {
        return Err(CodecError::TooManyEntries);
    }
    let (errors, next) = read_blob(bytes, 0)?;
    let (positional, next) = parse_positional(bytes, next, count)?;
    let (signals, next) = read_blob(bytes, next)?;
    let (keyed, _) = parse_keyed(bytes, next, max_entries - count)?;
    Ok(Container {
        errors: errors.to_vec(),
        positional,
        signals: signals.to_vec(),
        keyed,
    })
}

// ===========================================================================
// cipher
// ===========================================================================

const QR_ORDER: [[usize; 4]; 8] = [
    [0, 4, 8, 12],
    [1, 5, 9, 13],
    [2, 6, 10, 14],
    [3, 7, 11, 15],
    [0, 5, 10, 15],
    [1, 6, 11, 12],
    [2, 7, 8, 13],
    [3, 4, 9, 14],
];

fn quarter_round(state: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize) {
    state[a] = state[a].wrapping_add(state[b]);
    state[d] = (state[d] ^ state[a]).rotate_left(16);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] = (state[b] ^ state[c]).rotate_left(12);
    state[a] = state[a].wrapping_add(state[b]);
    state[d] = (state[d] ^ state[a]).rotate_left(8);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] = (state[b] ^ state[c]).rotate_left(7);
}

/// `applyStreamCipher` (target file line 3291). Symmetric: encrypts or decrypts.
pub fn crypt(data: &[u8], keys: &[u32; 8], seeds: [u32; 2], build: &Build) -> Vec<u8> {
    let mut out = vec![0u8; data.len()];
    let mut counter = build.counter;
    for (block_index, chunk) in data.chunks(64).enumerate() {
        let base = [
            build.ivs[0],
            build.ivs[1],
            build.ivs[2],
            build.ivs[3],
            keys[0],
            keys[1],
            keys[2],
            keys[3],
            keys[4],
            keys[5],
            keys[6],
            keys[7],
            counter as u32,
            (counter >> 32) as u32,
            seeds[0],
            seeds[1],
        ];
        let mut state = base;
        for _ in 0..build.rounds {
            for qr in QR_ORDER {
                quarter_round(&mut state, qr[0], qr[1], qr[2], qr[3]);
            }
        }
        for (word, initial) in state.iter_mut().zip(base.iter()) {
            *word = word.wrapping_add(*initial);
        }
        let offset = block_index * 64;
        for (index, byte) in chunk.iter().enumerate() {
            let word = state[index / 4];
            out[offset + index] = byte ^ ((word >> (8 * (index % 4))) as u8);
        }
        counter = (counter + 1) % (1u64 << 53);
    }
    out
}

// ===========================================================================
// base64
// ===========================================================================

fn alphabet_chars(alphabet: &str) -> Result<Vec<char>, CodecError> {
    let chars: Vec<char> = alphabet.chars().take(64).collect();
    if chars.len() < 64 {
        return Err(CodecError::ShortAlphabet(alphabet.chars().count()));
    }
    Ok(chars)
}

/// The launcher's base64: 3 bytes to 4 symbols, no padding.
pub fn base64_encode(bytes: &[u8], alphabet: &str) -> Result<String, CodecError> {
    let chars = alphabet_chars(alphabet)?;
    let mut out = String::new();
    let mut i = 0;
    while i + 3 <= bytes.len() {
        let b0 = bytes[i];
        let b1 = bytes[i + 1];
        let b2 = bytes[i + 2];
        out.push(chars[(b0 >> 2) as usize]);
        out.push(chars[(((b0 & 3) << 4) | (b1 >> 4)) as usize]);
        out.push(chars[(((b1 & 15) << 2) | (b2 >> 6)) as usize]);
        out.push(chars[(b2 & 63) as usize]);
        i += 3;
    }
    match bytes.len() - i {
        2 => {
            let b0 = bytes[i];
            let b1 = bytes[i + 1];
            out.push(chars[(b0 >> 2) as usize]);
            out.push(chars[(((b0 & 3) << 4) | (b1 >> 4)) as usize]);
            out.push(chars[((b1 & 15) << 2) as usize]);
        }
        1 => {
            let b0 = bytes[i];
            out.push(chars[(b0 >> 2) as usize]);
            out.push(chars[((b0 & 3) << 4) as usize]);
        }
        _ => {}
    }
    Ok(out)
}

/// Inverse of [`base64_encode`]. `=` is data when it is one of the 64 symbols
/// and padding otherwise (the session alphabet in the target capture puts `=`
/// at index 42).
pub fn base64_decode(text: &str, alphabet: &str) -> Result<Vec<u8>, CodecError> {
    let chars = alphabet_chars(alphabet)?;
    let mut lookup: HashMap<char, u8> = HashMap::new();
    for (index, ch) in chars.iter().enumerate() {
        lookup.insert(*ch, index as u8);
    }
    let mut values: Vec<u8> = Vec::new();
    for ch in text.chars() {
        if ch.is_whitespace() {
            continue;
        }
        match lookup.get(&ch) {
            Some(value) => values.push(*value),
            None if ch == '=' => continue,
            None => return Err(CodecError::BadBase64(ch)),
        }
    }
    if values.len() % 4 == 1 {
        return Err(CodecError::BadBase64Length(values.len()));
    }
    let mut out = Vec::with_capacity(values.len() / 4 * 3 + 2);
    let mut i = 0;
    while i + 4 <= values.len() {
        let d0 = values[i];
        let d1 = values[i + 1];
        let d2 = values[i + 2];
        let d3 = values[i + 3];
        out.push((d0 << 2) | (d1 >> 4));
        out.push(((d1 & 15) << 4) | (d2 >> 2));
        out.push(((d2 & 3) << 6) | d3);
        i += 4;
    }
    match values.len() - i {
        2 => out.push((values[i] << 2) | (values[i + 1] >> 4)),
        3 => {
            out.push((values[i] << 2) | (values[i + 1] >> 4));
            out.push(((values[i + 1] & 15) << 4) | (values[i + 2] >> 2));
        }
        _ => {}
    }
    Ok(out)
}

// ===========================================================================
// payload
// ===========================================================================

/// A decrypted payload with its framing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedPayload {
    /// The decrypted container.
    pub container: Vec<u8>,
    /// The two 32-bit seeds from the big-endian prefix.
    pub seeds: [u32; 2],
    /// The whole decoded byte string (prefix + ciphertext).
    pub raw: Vec<u8>,
}

/// `base64(be32(seed1) || be32(seed2) || ciphertext)`.
pub fn encode_payload(
    container: &[u8],
    session: &Session,
    seeds: [u32; 2],
    build: &Build,
) -> Result<String, CodecError> {
    let ciphertext = crypt(container, &session.keys, seeds, build);
    let mut raw = Vec::with_capacity(8 + ciphertext.len());
    raw.extend_from_slice(&seeds[0].to_be_bytes());
    raw.extend_from_slice(&seeds[1].to_be_bytes());
    raw.extend_from_slice(&ciphertext);
    base64_encode(&raw, &session.alphabet)
}

/// Decrypt a payload and expose its container.
pub fn decode_payload(
    payload: &str,
    session: &Session,
    build: &Build,
) -> Result<DecodedPayload, CodecError> {
    let raw = base64_decode(payload, &session.alphabet)?;
    if raw.len() < 8 {
        return Err(CodecError::ShortPayload(raw.len()));
    }
    let seeds = [
        u32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]),
        u32::from_be_bytes([raw[4], raw[5], raw[6], raw[7]]),
    ];
    let container = crypt(&raw[8..], &session.keys, seeds, build);
    Ok(DecodedPayload {
        container,
        seeds,
        raw,
    })
}

// ===========================================================================
// known-answer tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN: &str =
        "A4peOJGhAQAA_R6jhFOmE96X3pjkjUK24DtgrMqnzp4dlOASRMAInfR-00XoAUm92wCuclLXwH9eCOfvosJeCA==";
    const ALPHABET: &str = "768eqNwP4xRQJ0kacboBKn5j1ugh9SIiVvZsmrtMzF=_OlWD2UGp3AHE-dCLYTXyf";
    const KEYS: [u32; 8] = [
        1_944_786_431,
        1_181_771_095,
        2_061_591_115,
        263_772_366,
        1_195_508_087,
        1_094_872_937,
        60_797_880,
        642_840_060,
    ];

    fn session() -> Session {
        Session::new(TOKEN, ALPHABET, KEYS)
    }

    fn hex(text: &str) -> Vec<u8> {
        assert!(text.len().is_multiple_of(2), "odd hex length");
        (0..text.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&text[i..i + 2], 16).expect("hex"))
            .collect()
    }

    fn unhex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn token_hash_matches_the_capture() {
        assert_eq!(session().token_hash, 403_968_531);
        assert_eq!(session().token_hash, 0x1814_1213);
    }

    #[test]
    fn five_round_build_constants_are_the_file_literals() {
        let build = Build::TARGET_SEED_1;
        assert_eq!(
            build.ivs,
            [522_575_410, 361_402_133, 1_980_070_771, 3_031_302_865]
        );
        assert_eq!(build.counter, 307_069_178_630_199);
        assert_eq!(build.rounds, 5);
        // The file stores -1263664431 as an int32 literal.
        assert_eq!(build.ivs[3] as i32, -1_263_664_431);
    }

    #[test]
    fn keystream_matches_the_js_oracle() {
        // 64 bytes of mask from Keystream(0x18141213, 0x11223344 ^ 2363885845).
        let mut stream = Keystream::new(403_968_531, 0x1122_3344 ^ 2_363_885_845);
        let mut got = Vec::new();
        for _ in 0..64 {
            got.push(stream.next_byte());
        }
        let expected = hex(concat!(
            "c44ce96edd5caca9",
            "1052b18bb8f85ad7",
            "8ef4681d3af7811f",
            "3c24c87fe17bcf3c",
            "d60346545b065865",
            "9419aec0b69b8eea",
            "14ff5d11fe7c33bb",
            "15e74af62510f234",
        ));
        assert_eq!(got, expected);
    }

    #[test]
    fn writer_matches_the_js_oracle() {
        let extra = encode_signal(403_968_531, 0x1122_3344, 2_363_885_845, |w| {
            w.number(3.0);
            w.number(418.0);
            w.number(-7.0);
            w.byte(0);
            w.string("héllo \u{0} 😀");
            w.string("héllo \u{0} 😀");
            w.number(1.5);
            w.number(-2_147_483_648.0);
            w.byte(255);
        });
        assert_eq!(
            unhex(&extra),
            "11223344c76ee429dd346f007c3edeab78787a27116ce81dbbf7011f3c24c87fe183f05c5683c674a4"
        );
    }

    #[test]
    fn float_bytes_match_the_js_oracle() {
        let extra = encode_signal(403_968_531, 0x0a0b_0c0d, 0, |w| {
            w.number(f64::NAN);
            w.number(f64::INFINITY);
            w.number(f64::NEG_INFINITY);
            w.number(5e-324);
            w.number(2_147_483_648.0);
        });
        assert_eq!(
            unhex(&extra),
            "0a0b0c0d193407444c486409e01337dd4ebda1450b88d1b2c79609dbd1f63cdc7aaf830031319a10d0f88728556ac24a9d"
        );
    }

    #[test]
    fn number_writer_covers_every_branch() {
        let values = [
            0.0,
            -0.0,
            1.0,
            -1.0,
            31.0,
            32.0,
            -32.0,
            4095.0,
            4096.0,
            524_287.0,
            524_288.0,
            67_108_863.0,
            67_108_864.0,
            2_147_483_647.0,
            -2_147_483_648.0,
            2_147_483_648.0,
            -2_147_483_649.0,
            4_294_967_295.0,
            1.5,
            5e-324,
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ];
        let extra = encode_signal(403_968_531, 0x0a0b_0c0d, 0, |w| {
            for value in values {
                w.number(value);
            }
        });
        let (decoded, _) = decode_signal(&extra, 403_968_531, 0, |r| {
            values
                .iter()
                .map(|_| r.number())
                .collect::<Result<Vec<_>, _>>()
        })
        .expect("decode");
        for (got, want) in decoded.iter().zip(values.iter()) {
            if want.is_nan() {
                assert!(got.is_nan());
            } else {
                assert_eq!(got.to_bits(), want.to_bits(), "{want} -> {got}");
            }
        }
    }

    #[test]
    fn string_round_trip_including_interned_and_nul() {
        let strings = [
            "",
            "alpha",
            "alpha",
            "héllo",
            "日本語",
            "😀",
            "\u{0}",
            "a\u{0}b",
            "alpha",
            "",
        ];
        let extra = encode_signal(403_968_531, 7, 123, |w| {
            for value in strings {
                w.string(value);
            }
        });
        let (decoded, _) = decode_signal(&extra, 403_968_531, 123, |r| {
            strings
                .iter()
                .map(|_| r.string())
                .collect::<Result<Vec<_>, _>>()
        })
        .expect("decode");
        assert_eq!(decoded, strings);
    }

    #[test]
    fn cipher_matches_the_js_oracle() {
        let data = b"shape signal codec known-answer test";
        let ciphertext = crypt(
            data,
            &KEYS,
            [0x1122_3344, 0x5566_7788],
            &Build::TARGET_SEED_1,
        );
        assert_eq!(
            unhex(&ciphertext),
            "9261e0fd321a927f203fdd58566b6e29521bdeeddda00e2f8d654fa653a4dcc89be7b369"
        );
        // symmetric
        assert_eq!(
            crypt(
                &ciphertext,
                &KEYS,
                [0x1122_3344, 0x5566_7788],
                &Build::TARGET_SEED_1
            ),
            data
        );
    }

    #[test]
    fn base64_matches_the_js_oracle_and_treats_equals_as_data() {
        assert_eq!(ALPHABET.chars().count(), 65);
        assert_eq!(ALPHABET.chars().position(|c| c == '='), Some(42));

        let bytes = hex(
            "00070e151c232a31383f464d545b626970777e858c939aa1a8afb6bdc4cbd2d9e0e7eef5fc030a11181f262d343b424950575e656c737a81888f969da4abb2b9c0c7",
        );
        let text = base64_encode(&bytes, ALPHABET).expect("encode");
        assert_eq!(
            text,
            "779kNb2sRsq-a3u0nNlZgj6Eiz5Jmd=v=RXHD9BQ3lMVdXLAy7JRqbVixZ33k3xxKNSIu5UpIzw4sdgSFRWGW9eP"
        );
        assert_eq!(base64_decode(&text, ALPHABET).expect("decode"), bytes);
    }

    // The XHR signal program.
    #[derive(Debug, PartialEq)]
    struct Xhr {
        responses: Vec<f64>,
        cp_hash: f64,
        status: f64,
        load_end: f64,
        send: f64,
        has_network_error: bool,
        response: String,
    }

    fn write_xhr(w: &mut SignalWriter, xhr: &Xhr) {
        w.number(xhr.responses.len() as f64);
        for value in xhr.responses.iter().rev() {
            w.number(*value);
        }
        w.number(xhr.cp_hash);
        w.number(xhr.status);
        w.number(xhr.load_end);
        w.number(xhr.send);
        w.byte(if xhr.has_network_error { 76 } else { 0 });
        w.string(&xhr.response);
    }

    fn read_xhr(r: &mut SignalReader<'_>) -> Result<Xhr, CodecError> {
        let count = r.number()? as usize;
        let mut responses = Vec::with_capacity(count);
        for _ in 0..count {
            responses.push(r.number()?);
        }
        responses.reverse();
        Ok(Xhr {
            responses,
            cp_hash: r.number()?,
            status: r.number()?,
            load_end: r.number()?,
            send: r.number()?,
            has_network_error: r.byte()? == 76,
            response: r.string()?,
        })
    }

    #[test]
    fn signal_matches_the_js_oracle() {
        let xhr = Xhr {
            responses: vec![418.0, 3.5, -7.0],
            cp_hash: 123_456_789.0,
            status: 200.0,
            load_end: 1_699_999_999_999.0,
            send: 1_699_999_999_000.0,
            has_network_error: false,
            response: "{\"ok\":true}".to_owned(),
        };
        let extra = encode_signal(403_968_531, 0x1122_3344, 2_363_885_845, |w| {
            write_xhr(w, &xhr)
        });
        assert_eq!(
            unhex(&extra),
            "11223344c70b696edd5caca9105ef1a9b5cdb26b65f5401bbaf771606ada7407a3fbcfbc9755b8e82344581eb676c5e28ceffc9f71825d"
        );
        let (decoded, seed) =
            decode_signal(&extra, 403_968_531, 2_363_885_845, read_xhr).expect("decode");
        assert_eq!(seed, 0x1122_3344);
        assert_eq!(decoded, xhr);
    }

    #[test]
    fn payload_matches_the_js_oracle() {
        let container = hex(
            "00003711223344c70b696edd5caca9105ef1a9b5cdb26b65f5401bbaf771606ada7407a3fbcfbc9755b8e82344581eb676c5e28ceffc9f71825d00",
        );
        let payload = encode_payload(
            &container,
            &session(),
            [0x0a0b_0c0d, 0x0102_0304],
            &Build::TARGET_SEED_1,
        )
        .expect("encode");
        assert_eq!(
            payload,
            "8VOJecq872ons2cBbwKY7agUiLp5l3u2x76D7dXH_gBDm_MJtYTvj4qBOZkAmi6FQtl69vscW6H5kwyxyCkpi9kJW7"
        );

        let decoded = decode_payload(&payload, &session(), &Build::TARGET_SEED_1).expect("decode");
        assert_eq!(decoded.seeds, [0x0a0b_0c0d, 0x0102_0304]);
        assert_eq!(decoded.container, container);

        let parsed = unpack_container(&decoded.container, Some(1)).expect("unpack");
        let extra = &parsed.positional[0].as_ref().expect("entry").extra;
        let (xhr, seed) =
            decode_signal(extra, session().token_hash, 2_363_885_845, read_xhr).expect("signal");
        assert_eq!(seed, 0x1122_3344);
        assert_eq!(xhr.response, "{\"ok\":true}");
        assert_eq!(xhr.status, 200.0);
    }

    #[test]
    fn container_round_trips_with_sentinel_and_auto_count() {
        let container = Container {
            errors: vec![],
            positional: vec![
                Some(PositionalEntry {
                    encoded: vec![],
                    extra: vec![1, 2, 3, 4, 5],
                }),
                None,
                Some(PositionalEntry {
                    encoded: vec![],
                    extra: vec![9],
                }),
            ],
            signals: vec![0x31, 0x32],
            keyed: vec![KeyedEntry {
                name: 7,
                encoded: vec![],
                extra: vec![0xaa],
            }],
        };
        let packed = pack_container(&container);
        let parsed = unpack_container(&packed, None).expect("auto count");
        assert_eq!(parsed, container);
        assert_eq!(
            unpack_container(&packed, Some(3)).expect("fixed"),
            container
        );
    }

    #[test]
    fn errors_are_reported() {
        assert!(matches!(
            SignalReader::new(&[0, 0, 0], 0, 0),
            Err(CodecError::ShortSeedPrefix(3))
        ));
        let mut reader = SignalReader::new(&[0, 0, 0, 1], 0, 0).expect("prefix");
        assert!(matches!(reader.number(), Err(CodecError::Truncated(_))));
        assert!(matches!(
            base64_decode("!!!!", ALPHABET),
            Err(CodecError::BadBase64('!'))
        ));
        assert!(matches!(
            base64_encode(&[0], "too short"),
            Err(CodecError::ShortAlphabet(_))
        ));
    }
}
