//! Final header formatting only: numeric a chunks, b/f session binding, opaque
//! c/d/z context. No signal interpretation or captured build defaults.

use std::{collections::BTreeMap, fmt};

use serde::{de, ser::SerializeMap, Deserialize, Deserializer, Serialize, Serializer};

use super::{invalid, validate_header_name, Error, Generator, Payload, Profile, MAX_HEADER_BYTES};
use crate::signals::codec::hash_string_token;

pub const MAX_HEADER_ENTRIES: usize = 65_536;
const MAX_CONTEXT_BYTES: usize = 64 * 1024;
const MAX_BUNDLE_BYTES: usize = MAX_HEADER_BYTES + 5 * MAX_CONTEXT_BYTES;

/// Explicit presence of computed b/f headers, and caller-provided opaque c/d/z
/// values. No checksum, token, descriptor, or build marker is invented.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeaderContext {
    #[serde(default)]
    pub b: bool,
    #[serde(default)]
    pub f: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub c: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub d: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub z: Option<String>,
}

/// Bounded header map with case-insensitive duplicate rejection, including
/// exact duplicate JSON keys. Original name spelling is retained for serialization.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Headers {
    entries: BTreeMap<String, (String, String)>,
    bytes: usize,
}

impl Headers {
    /// Construct a single header (the original byte-codec convenience API).
    pub fn new(name: String, value: String) -> Result<Self, Error> {
        Self::from_entries([(name, value)])
    }

    pub fn from_entries(
        entries: impl IntoIterator<Item = (String, String)>,
    ) -> Result<Self, Error> {
        let mut headers = Self::default();
        for (name, value) in entries {
            headers.insert(name, value)?;
        }
        Ok(headers)
    }

    fn insert(&mut self, name: String, value: String) -> Result<(), Error> {
        validate_header_name(&name)?;
        if value.len() > MAX_HEADER_BYTES || !value.bytes().all(|b| (0x20..=0x7e).contains(&b)) {
            return Err(invalid(
                "header value must be bounded visible ASCII without control characters",
            ));
        }
        let key = name.to_ascii_lowercase();
        if self.entries.contains_key(&key) {
            return Err(invalid("duplicate header name (case-insensitive)"));
        }
        if self.entries.len() >= MAX_HEADER_ENTRIES
            || self.bytes + name.len() + value.len() > MAX_BUNDLE_BYTES
        {
            return Err(invalid("header bundle exceeds the entry/total size limit"));
        }
        self.bytes += name.len() + value.len();
        self.entries.insert(key, (name, value));
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.entries
            .get(&name.to_ascii_lowercase())
            .map(|(_, value)| value.as_str())
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.entries
            .values()
            .map(|(name, value)| (name.as_str(), value.as_str()))
    }

    /// Single-entry view; None for a bundle or empty map.
    pub fn as_parts(&self) -> Option<(&str, &str)> {
        if self.entries.len() == 1 {
            self.iter().next()
        } else {
            None
        }
    }
}

impl Serialize for Headers {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.entries.len()))?;
        for (name, value) in self.iter() {
            map.serialize_entry(name, value)?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for Headers {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> de::Visitor<'de> for Visitor {
            type Value = Headers;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a map of supported header names to string values")
            }
            fn visit_map<M: de::MapAccess<'de>>(self, mut map: M) -> Result<Headers, M::Error> {
                let mut headers = Headers::default();
                while let Some((name, value)) = map.next_entry::<String, String>()? {
                    headers.insert(name, value).map_err(de::Error::custom)?;
                }
                if headers.entries.is_empty() {
                    return Err(de::Error::custom("missing header"));
                }
                Ok(headers)
            }
        }
        deserializer.deserialize_map(Visitor)
    }
}

pub(super) fn validate_profile(profile: &Profile) -> Result<(), Error> {
    // Leave room for the longest supported numeric suffix on an a header.
    if profile.chunk_size.is_some() && profile.header_name.len() > 251 {
        return Err(invalid("chunked header name must be at most 251 bytes"));
    }
    if let Some(size) = profile.chunk_size {
        if !(1..=MAX_HEADER_BYTES).contains(&size) {
            return Err(invalid("chunk_size must be in 1..=MAX_HEADER_BYTES"));
        }
    }
    if profile
        .transaction_token
        .as_ref()
        .is_some_and(|token| token.len() > MAX_CONTEXT_BYTES)
    {
        return Err(invalid("transaction_token exceeds 64 KiB"));
    }
    if let Some(context) = &profile.header_context {
        validate_context(context, profile.transaction_token.as_deref())?;
    }
    Ok(())
}

fn validate_context(context: &HeaderContext, token: Option<&str>) -> Result<(), Error> {
    if context.b || context.f {
        let token =
            token.ok_or_else(|| invalid("transaction_token is required for b/f headers"))?;
        if context.f {
            validate_context_value(token)?;
        }
    }
    for value in [&context.c, &context.d, &context.z].into_iter().flatten() {
        validate_context_value(value)?;
    }
    Ok(())
}

fn validate_context_value(value: &str) -> Result<(), Error> {
    if value.len() > MAX_CONTEXT_BYTES || !value.bytes().all(|b| (0x20..=0x7e).contains(&b)) {
        return Err(invalid(
            "header context must be at most 64 KiB of visible ASCII without control characters",
        ));
    }
    Ok(())
}

/// Captured stringBufferHash(token + FULL unchunked payload).toString(36).
/// Signed i32, wrapping multiply/add, over pairs of UTF-16 units. This checksum
/// is not authentication. Token hashing here never masks plaintext sections.
pub fn header_checksum(token: &str, payload: &str) -> String {
    let hash = i64::from(hash_string_token(&format!("{token}{payload}")));
    let mut magnitude = hash.unsigned_abs();
    let mut out = Vec::new();
    loop {
        out.push(b"0123456789abcdefghijklmnopqrstuvwxyz"[(magnitude % 36) as usize]);
        magnitude /= 36;
        if magnitude == 0 {
            break;
        }
    }
    if hash < 0 {
        out.push(b'-');
    }
    out.reverse();
    String::from_utf8(out).expect("base36 ASCII")
}

impl Generator {
    pub(super) fn context(&self, supplied: Option<&HeaderContext>) -> Result<HeaderContext, Error> {
        if let (Some(profile), Some(payload)) = (&self.profile.header_context, supplied) {
            if profile != payload {
                return Err(invalid("conflicting profile/payload header_context"));
            }
        }
        let context = supplied
            .or(self.profile.header_context.as_ref())
            .cloned()
            .unwrap_or_default();
        validate_context(&context, self.profile.transaction_token.as_deref())?;
        Ok(context)
    }

    /// Encrypt bytes, split using the explicit chunk policy, and emit only the
    /// sibling headers selected by payload/profile context.
    pub fn encode_headers(&self, payload: &Payload) -> Result<Headers, Error> {
        let value = self.encode_bytes(&payload.plaintext()?, payload.seeds)?;
        self.encode_finalized_headers(&value, payload.header_context.as_ref())
    }

    /// Wrap canonical finalized custom-base64 without packing or encrypting it.
    pub fn encode_finalized_headers(
        &self,
        value: &str,
        context: Option<&HeaderContext>,
    ) -> Result<Headers, Error> {
        let context = self.context(context)?;
        self.codec.validate_finalized_payload(value)?;
        let size = self.profile.chunk_size.unwrap_or(value.len());
        let count = value.len().div_ceil(size);
        let siblings = usize::from(context.b)
            + usize::from(context.f)
            + [&context.c, &context.d, &context.z]
                .into_iter()
                .filter(|v| v.is_some())
                .count();
        if count + siblings > MAX_HEADER_ENTRIES {
            return Err(invalid("payload requires too many header chunks"));
        }
        let mut headers = Headers::default();
        for (index, chunk) in value.as_bytes().chunks(size).enumerate() {
            let name = if index == 0 {
                self.profile.header_name.clone()
            } else {
                format!("{}{}", self.profile.header_name, index - 1)
            };
            headers.insert(
                name,
                String::from_utf8(chunk.to_vec()).expect("validated ASCII alphabet"),
            )?;
        }
        let prefix = &self.profile.header_name[..self.profile.header_name.len() - 1];
        let token = self.profile.transaction_token.as_deref();
        if context.b {
            headers.insert(
                format!("{prefix}b"),
                header_checksum(token.expect("validated token"), &value),
            )?;
        }
        if context.f {
            headers.insert(
                format!("{prefix}f"),
                token.expect("validated token").to_owned(),
            )?;
        }
        for (suffix, value) in [("c", context.c), ("d", context.d), ("z", context.z)] {
            if let Some(value) = value {
                headers.insert(format!("{prefix}{suffix}"), value)?;
            }
        }
        Ok(headers)
    }

    /// Join numeric chunks, verify any b/f headers, and decrypt to opaque bytes.
    /// Context preserves exact c/d/z values and the presence of b/f headers.
    pub fn decode_headers(&self, headers: &Headers) -> Result<Payload, Error> {
        let (value, context) = self.decode_finalized_headers(headers)?;
        let mut payload = self.decode(&value)?;
        if context != HeaderContext::default() {
            payload.header_context = Some(context);
        }
        Ok(payload)
    }

    /// Join and check the configured family before any decryption. Never trusts
    /// an f header over the explicit profile token. Returns even absent context.
    pub fn decode_finalized_headers(
        &self,
        headers: &Headers,
    ) -> Result<(String, HeaderContext), Error> {
        let base = headers.get(&self.profile.header_name).ok_or_else(|| {
            invalid("missing configured a header (unsupported header name or incomplete bundle)")
        })?;
        let prefix =
            self.profile.header_name[..self.profile.header_name.len() - 1].to_ascii_lowercase();
        let mut chunks = BTreeMap::new();
        chunks.insert(0usize, base);
        let mut context = HeaderContext::default();
        let mut checksum = None;
        let mut supplied_token = None;
        for (name, value) in headers.iter() {
            let name = name.to_ascii_lowercase();
            let suffix = name
                .strip_prefix(&prefix)
                .ok_or_else(|| invalid("unsupported header name or family"))?;
            match suffix {
                "a" => {}
                "b" => {
                    context.b = true;
                    checksum = Some(value);
                }
                "f" => {
                    context.f = true;
                    supplied_token = Some(value);
                }
                "c" => context.c = Some(value.to_owned()),
                "d" => context.d = Some(value.to_owned()),
                "z" => context.z = Some(value.to_owned()),
                other if other.starts_with('a') => {
                    if self.profile.chunk_size.is_none() {
                        return Err(invalid("a chunks require an explicit chunk_size"));
                    }
                    let index = &other[1..];
                    if index.is_empty()
                        || !index.bytes().all(|b| b.is_ascii_digit())
                        || (index.len() > 1 && index.starts_with('0'))
                    {
                        return Err(invalid("invalid a chunk suffix (no leading zero aliases)"));
                    }
                    let index = index
                        .parse::<usize>()
                        .ok()
                        .and_then(|n| n.checked_add(1))
                        .filter(|&n| n < MAX_HEADER_ENTRIES)
                        .ok_or_else(|| invalid("a chunk index exceeds the limit"))?;
                    chunks.insert(index, value);
                }
                _ => return Err(invalid("unsupported additional header name or family")),
            }
        }
        let mut value = String::new();
        let count = chunks.len();
        for (expected, (index, chunk)) in chunks.into_iter().enumerate() {
            if index != expected {
                return Err(invalid("missing a chunk / noncontiguous numeric suffixes"));
            }
            if let Some(size) = self.profile.chunk_size {
                if chunk.is_empty()
                    || chunk.len() > size
                    || (expected + 1 < count && chunk.len() != size)
                {
                    return Err(invalid(
                        "a chunk length does not match the explicit chunk_size",
                    ));
                }
            }
            if value.len() + chunk.len() > MAX_HEADER_BYTES {
                return Err(invalid("joined a payload exceeds the size limit"));
            }
            value.push_str(chunk);
        }
        self.context(Some(&context))?;
        let token = self.profile.transaction_token.as_deref();
        if let Some(f) = supplied_token {
            if Some(f) != token {
                return Err(invalid("f header does not match transaction_token"));
            }
        }
        if let Some(b) = checksum {
            if b != header_checksum(token.expect("validated token"), &value) {
                return Err(invalid(
                    "b checksum mismatch for transaction_token + full payload",
                ));
            }
        }
        self.codec.validate_finalized_payload(&value)?;
        Ok((value, context))
    }
}

/// Header-report JSON extension. All six report fields remain at the root;
/// `header_context` records sibling presence even when every sibling is absent.
/// The payload-only [`super::report::Report`] schema remains unchanged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeaderReport {
    pub errors: Vec<super::superpack::Value>,
    #[serde(rename = "positionalSignals")]
    pub positional_signals: Vec<Option<super::report::Entry>>,
    pub signals: Vec<super::superpack::Value>,
    #[serde(rename = "keyedSignals")]
    pub keyed_signals: Vec<(u32, Option<super::report::Entry>)>,
    pub seeds: [u32; 2],
    pub keysets: Vec<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header_context: Option<HeaderContext>,
}

impl From<super::report::Report> for HeaderReport {
    fn from(report: super::report::Report) -> Self {
        Self {
            errors: report.errors,
            positional_signals: report.positional_signals,
            signals: report.signals,
            keyed_signals: report.keyed_signals,
            seeds: report.seeds,
            keysets: report.keysets,
            header_context: None,
        }
    }
}

impl HeaderReport {
    /// Project only the four report channels, dropping seeds, keysets and the
    /// opaque header context.
    /// Build an encodable header report from the four-channel document.
    pub fn from_channels(
        channels: &super::report::ChannelReport,
        seeds: [u32; 2],
        keysets: Vec<Vec<String>>,
        header_context: Option<HeaderContext>,
    ) -> Self {
        Self {
            errors: channels.errors.clone(),
            positional_signals: channels.positional_signals.clone(),
            signals: channels.signals.clone(),
            keyed_signals: channels.keyed_signals.clone(),
            seeds,
            keysets,
            header_context,
        }
    }

    /// Owned four-channel document, directly encodable.
    pub fn channels_owned(&self) -> super::report::ChannelReport {
        super::report::ChannelReport {
            errors: self.errors.clone(),
            positional_signals: self.positional_signals.clone(),
            signals: self.signals.clone(),
            keyed_signals: self.keyed_signals.clone(),
        }
    }

    pub fn channels(&self) -> super::report::Channels<'_> {
        super::report::Channels {
            errors: &self.errors,
            positional_signals: &self.positional_signals,
            signals: &self.signals,
            keyed_signals: &self.keyed_signals,
        }
    }

    /// Bounded typed JSON parser, including duplicate/unknown field and trailing
    /// JSON rejection. Supports the same deep values as report CLI input.
    pub fn from_json(input: &[u8]) -> Result<Self, Error> {
        if input.len() > 128 * 1024 * 1024 {
            return Err(invalid("header report JSON exceeds 128 MiB"));
        }
        super::typed_json::parse_typed_json(input).map_err(|e| Error::Invalid(e.to_string()))
    }

    pub fn report(&self) -> super::report::Report {
        super::report::Report {
            errors: self.errors.clone(),
            positional_signals: self.positional_signals.clone(),
            signals: self.signals.clone(),
            keyed_signals: self.keyed_signals.clone(),
            seeds: self.seeds,
            keysets: self.keysets.clone(),
        }
    }
}

impl Generator {
    /// Pack the report once, finalize once, then use shared header formatting.
    pub fn encode_report_headers(&self, document: &HeaderReport) -> Result<Headers, Error> {
        self.context(document.header_context.as_ref())?;
        let value = self.codec.encode_report(&document.report())?;
        self.encode_finalized_headers(&value, document.header_context.as_ref())
    }

    /// Check headers first, then decode the final report with external facts.
    pub fn decode_report_headers(
        &self,
        headers: &Headers,
        positional_count: usize,
        keysets: Vec<Vec<String>>,
    ) -> Result<HeaderReport, Error> {
        let (value, context) = self.decode_finalized_headers(headers)?;
        let mut document: HeaderReport = self
            .codec
            .decode_report(&value, positional_count, keysets)?
            .into();
        document.header_context = Some(context);
        Ok(document)
    }
}

impl Headers {
    /// Parse a strict header-family block. LF or CRLF must be consistent. One
    /// optional final empty line is allowed, but bodies, start lines, folding,
    /// whitespace before the colon, and controls in values are refused. Only
    /// surrounding HTTP SP/HTAB is trimmed; interior bytes are untouched.
    /// Family selection/validation belongs to Generator's strict decoder.
    pub fn from_text(text: &str) -> Result<Self, Error> {
        if text.len() > MAX_BUNDLE_BYTES + MAX_HEADER_ENTRIES * 4 {
            return Err(invalid("header text exceeds the size limit"));
        }
        let crlf = text.contains("\r\n");
        let text = if crlf {
            // Validate before normalizing delimiters, never normalize values.
            if text.replace("\r\n", "").contains(['\r', '\n']) {
                return Err(invalid("inconsistent header line endings"));
            }
            text.replace("\r\n", "\n")
        } else {
            if text.contains('\r') {
                return Err(invalid("bare CR in header text"));
            }
            text.to_owned()
        };
        let text = text.strip_suffix('\n').unwrap_or(&text);
        let text = text.strip_suffix('\n').unwrap_or(text);
        let mut headers = Self::default();
        for line in text.split('\n') {
            let (name, value) = line
                .split_once(':')
                .ok_or_else(|| invalid("expected Name: value header line (no start line/body)"))?;
            headers.insert(name.to_owned(), value.trim_matches([' ', '\t']).to_owned())?;
        }
        Ok(headers)
    }

    /// Emit a family-only LF block. JSON is required to preserve opaque values
    /// with meaningful leading/trailing spaces (HTTP text trims optional space).
    pub fn to_text(&self) -> String {
        let mut text = String::new();
        for (name, value) in self.iter() {
            text.push_str(name);
            text.push_str(": ");
            text.push_str(value);
            text.push('\n');
        }
        text
    }
}
