//! Source-derived generic value packing, independent of signals and outer framing.
//!
//! See `docs/generator/superpack.md` for the supported subset and wire provenance.
mod decode;
mod encode;

use serde::{Deserialize, Serialize};

pub const DEPTH_EXTENSION: u32 = 0xdead_beef;

/// JSON-safe IEEE-754 representation. Bits preserve -0 and every NaN payload.
/// The source encoder normalizes NaNs to its fixed float32 quiet NaN.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Number(pub u64);
impl Serialize for Number {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&format!("{:016x}", self.0))
    }
}
impl<'de> Deserialize<'de> for Number {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let bits = String::deserialize(deserializer)?;
        if bits.len() != 16 || !bits.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(serde::de::Error::custom(
                "number requires 16 hexadecimal IEEE-754 bit digits",
            ));
        }
        u64::from_str_radix(&bits, 16)
            .map(Self)
            .map_err(serde::de::Error::custom)
    }
}
impl From<f64> for Number {
    fn from(value: f64) -> Self {
        Self(value.to_bits())
    }
}
impl Number {
    pub fn as_f64(self) -> f64 {
        f64::from_bits(self.0)
    }
}

/// Objects retain supplied entry order; packing uses source `.sort()` UTF-16
/// key order. Duplicate own keys are rejected. No JS prototypes or callbacks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Value {
    Undefined,
    Null,
    Bool(bool),
    Number(Number),
    String(String),
    Array(Vec<Value>),
    Object(Vec<(String, Value)>),
    Binary(Vec<u8>),
    /// Proven injective Date subset: integer milliseconds in 0..2^48.
    Date(u64),
    MissingSentinel,
    /// Wire extension, not a JavaScript detector/serializer callback.
    Extension {
        id: u32,
        payload: Box<Value>,
    },
}
impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Self::Number(v.into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Limits {
    pub max_bytes: usize,
    pub max_entries: usize,
    pub max_depth: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_bytes: 16 * 1024 * 1024,
            max_entries: 65_536,
            max_depth: 128,
        }
    }
}

/// Source options; resource limits are independent of semantic depth truncation.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SerializerOptions {
    pub keysets_to_omit: Vec<Vec<String>>,
    pub depth_bound: Option<usize>,
    pub missing_sentinel_extension: bool,
    /// Permit explicit opaque wire extensions. Never invokes JS or coerces data.
    pub opaque_extensions: bool,
    pub limits: Limits,
}
impl SerializerOptions {
    /// Options used for outer `.signals`; errors/entry `.encoded` use defaults.
    pub fn outer_channel(keysets_to_omit: Vec<Vec<String>>) -> Self {
        Self {
            depth_bound: Some(100),
            missing_sentinel_extension: true,
            keysets_to_omit,
            ..Self::default()
        }
    }
    pub(super) fn validate(&self) -> Result<(), Error> {
        if self.limits.max_depth > 256
            || self.limits.max_bytes > 16 * 1024 * 1024
            || self.limits.max_entries > 1_000_000
        {
            return Err(err(
                "limits exceed hard safety ceilings (depth 256, bytes 16 MiB, entries 1000000)",
            ));
        }
        if self.keysets_to_omit.len() > self.limits.max_entries {
            return Err(err("too many preloaded keysets"));
        }
        let mut count = 0usize;
        let mut bytes = 0usize;
        for keys in &self.keysets_to_omit {
            count = count.saturating_add(keys.len());
            if count > self.limits.max_entries {
                return Err(err("preloaded keysets exceed entry limit"));
            }
            let mut seen = std::collections::HashSet::new();
            for k in keys {
                bytes = bytes.saturating_add(k.len());
                if bytes > self.limits.max_bytes {
                    return Err(err("preloaded keysets exceed byte limit"));
                }
                if !seen.insert(k) {
                    return Err(err("duplicate preloaded key"));
                }
            }
        }
        if count > self.limits.max_entries || bytes > self.limits.max_bytes {
            return Err(err("preloaded keysets exceed limits"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackRequest {
    pub value: Value,
    #[serde(default)]
    pub options: SerializerOptions,
}
impl PackRequest {
    pub fn pack(&self) -> Result<Vec<u8>, Error> {
        pack(&self.value, &self.options)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnpackRequest {
    pub bytes: Vec<u8>,
    #[serde(default)]
    pub options: SerializerOptions,
}
impl UnpackRequest {
    pub fn unpack(&self) -> Result<Decoded, Error> {
        unpack(&self.bytes, &self.options)
    }
}

/// Structural inverse inferred from encoder output, not a recovered JS decoder.
/// Original bytes are retained even if a legal alternate encoding normalizes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decoded {
    pub value: Value,
    pub original_bytes: Vec<u8>,
    pub canonical: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("superpack: {0}")]
pub struct Error(pub String);
fn err(s: &str) -> Error {
    Error(s.into())
}

pub fn pack(value: &Value, options: &SerializerOptions) -> Result<Vec<u8>, Error> {
    options.validate()?;
    encode::pack(value, options)
}
/// Consume exactly one complete document. Noncanonical inputs are surfaced by
/// `canonical=false`; `original_bytes` always provides byte preservation.
pub fn unpack(bytes: &[u8], options: &SerializerOptions) -> Result<Decoded, Error> {
    options.validate()?;
    let value = decode::unpack(bytes, options)?;
    let canonical = pack(&value, options).is_ok_and(|b| b == bytes);
    Ok(Decoded {
        value,
        original_bytes: bytes.to_vec(),
        canonical,
    })
}
/// Require canonical source bytes as well as structural validity.
pub fn unpack_canonical(bytes: &[u8], options: &SerializerOptions) -> Result<Value, Error> {
    let result = unpack(bytes, options)?;
    if !result.canonical {
        return Err(err(
            "noncanonical input; use unpack to retain original bytes",
        ));
    }
    Ok(result.value)
}

fn utf16_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}
fn index_key(s: &str) -> Option<u32> {
    let n = s.parse::<u32>().ok()?;
    (n != u32::MAX && n.to_string() == s).then_some(n)
}
fn inherited_hist_key(s: &str) -> bool {
    matches!(
        s,
        "__proto__"
            | "constructor"
            | "__defineGetter__"
            | "__defineSetter__"
            | "hasOwnProperty"
            | "__lookupGetter__"
            | "__lookupSetter__"
            | "isPrototypeOf"
            | "propertyIsEnumerable"
            | "toString"
            | "valueOf"
            | "toLocaleString"
    )
}
