//! Reusable, source-compiled named-signal decoding.
//!
//! Compilation uses the existing explicit-initializer source-format adapter.
//! It does not claim the strict wire loader's independent lifecycle certification.
//! Runtime decoding loads recipes and session values; it never parses source.
mod compact;
/// Opt-in source-format compiler used by the CLI and SDK direct workflow.
#[doc(hidden)]
pub mod direct;
#[doc(hidden)]
pub mod source_compiler;
#[doc(hidden)]
pub mod source_counts;
pub use compact::{CompactSeedSchema, COMPACT_SEED_SCHEMA_FORMAT};

use super::{
    analysis::{analyze_program_signals, SignalFinderOptions},
    codec::hash_string_token,
    payload::{
        extract::{pin, SeedLayout, Session},
        superpack, HeaderReport, Headers, Report,
    },
    wire::*,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const ARTIFACT_FORMAT: &str = "shape-decode-artifact/1";

#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[error("invalid decode input: {0}")]
    Input(String),
    #[error("signal {position}: {reason}")]
    Signal { position: usize, reason: String },
}
type Result<T> = std::result::Result<T, DecodeError>;
fn input(e: impl std::fmt::Display) -> DecodeError {
    DecodeError::Input(e.to_string())
}
fn ensure(ok: bool, reason: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(input(reason))
    }
}

/// Adapter boundary: site identity plus suffixes a..z and numeric a chunks.
/// `Headers` rejects duplicate keys (including case variants), controls and oversize values.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CaptureHeaders {
    pub site_id: String,
    pub headers: Headers,
}
impl CaptureHeaders {
    pub fn validate(&self) -> Result<()> {
        if self.site_id.trim().is_empty() {
            return Err(input("siteId must not be empty"));
        }
        for (name, _) in self.headers.iter() {
            let n = name.to_ascii_lowercase();
            if !(n.len() == 1 && n.as_bytes()[0].is_ascii_lowercase()
                || n.strip_prefix('a')
                    .is_some_and(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())))
            {
                return Err(input(
                    "header keys must be suffixes a..z or numeric a chunks",
                ));
            }
        }
        Ok(())
    }
    pub fn expanded(&self, prefix: &str) -> Result<Headers> {
        self.validate()?;
        Headers::from_entries(
            self.headers
                .iter()
                .map(|(k, v)| (format!("{prefix}{}", k.to_ascii_lowercase()), v.to_owned())),
        )
        .map_err(input)
    }
}

/// One seed's pinned payload configuration and compiled, source-derived signal schema.
/// Session values are defaults and may be replaced together for another capture.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DecodeArtifact {
    /// Explicit non-lifecycle source-format recipes; absent in legacy artifacts.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub direct: bool,
    pub format: String,
    pub site_id: String,
    pub layout: SeedLayout,
    pub schema: SchemaDocument,
    pub float_byte_order: FloatByteOrder,
}

/// Public slim view: only named field objects appear in positionalSignals.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DecodedPayload {
    pub site_id: String,
    pub errors: Vec<superpack::Value>,
    pub positional_signals: BTreeMap<String, Map<String, Value>>,
    pub signals: Vec<superpack::Value>,
    pub keyed_signals: Vec<(u32, Option<super::payload::report::Entry>)>,
    pub seeds: [u32; 2],
    pub keysets: Vec<Vec<String>>,
    pub metadata: DecodeMetadata,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DecodeMetadata {
    pub original_report: HeaderReport,
    pub signals: BTreeMap<String, NamedSignal>,
    pub decoded: usize,
    pub missing: usize,
    pub exact_reencode: bool,
    pub float_byte_order: FloatByteOrder,
}

/// An artifact validated once and then immutable, for repeated decoding.
///
/// [`DecodeArtifact::decode`] re-validates on every call because its fields are
/// public and may change between calls. This type owns the artifact privately,
/// so validation on construction covers every later decode.
#[derive(Clone, Debug)]
pub struct ValidatedArtifact {
    artifact: DecodeArtifact,
    /// Each wire slot's constant metadata, serialized once on load.
    constants: Vec<Map<String, Value>>,
    number_policies: source_number::Policies,
}

impl ValidatedArtifact {
    pub fn new(artifact: DecodeArtifact) -> Result<Self> {
        artifact.validate()?;
        Ok(Self::validated(artifact))
    }
    /// Parse and validate once (`DecodeArtifact::from_json` validates).
    pub fn from_json(bytes: &[u8]) -> Result<Self> {
        Ok(Self::validated(DecodeArtifact::from_json(bytes)?))
    }
    fn validated(artifact: DecodeArtifact) -> Self {
        let constants = artifact
            .schema
            .wire
            .as_ref()
            .map(|w| w.slots.iter().map(named::slot_constants).collect())
            .unwrap_or_default();
        let number_policies =
            source_number::validate(&artifact.schema).expect("validated source-number policies");
        Self {
            artifact,
            constants,
            number_policies,
        }
    }
    pub fn artifact(&self) -> &DecodeArtifact {
        &self.artifact
    }
    pub fn into_inner(self) -> DecodeArtifact {
        self.artifact
    }
    pub fn decode(
        &self,
        capture: &CaptureHeaders,
        session: Option<&Session>,
        options: DecodeOptions,
    ) -> Result<DecodedPayload> {
        self.artifact.decode_validated(
            capture,
            session,
            options,
            Some(&self.constants),
            &self.number_policies,
        )
    }

    /// Named signals → header bundle.
    ///
    /// The payload is either a full decoded payload (`positionalSignals` plus
    /// `metadata.originalReport`, whose retained bytes supply every fact the
    /// display does not describe: random prefixes, unprojected leaves and
    /// flush arms) or a bare `positionalSignals` object. Every signal is
    /// re-decoded from the bytes the encoder produces and must project back to
    /// exactly the requested fields; a signal that cannot round-trip is an
    /// error, never silently different bytes.
    pub fn encode_named(
        &self,
        payload: &Value,
        session: Option<&Session>,
    ) -> Result<CaptureHeaders> {
        let object = payload
            .as_object()
            .ok_or_else(|| input("encode payload must be a JSON object"))?;
        if let Some(site) = object.get("siteId") {
            let site = site
                .as_str()
                .ok_or_else(|| input("\"siteId\" must be a string"))?;
            if site != self.artifact.site_id {
                return Err(input("payload/artifact siteId mismatch"));
            }
        }
        let signals = match object.get("positionalSignals") {
            Some(Value::Object(map)) => map,
            Some(_) => return Err(input("\"positionalSignals\" must be an object")),
            None => object,
        };
        let original: Option<HeaderReport> = match object
            .get("metadata")
            .and_then(|metadata| metadata.get("originalReport"))
        {
            Some(value) => Some(
                serde_json::from_value(value.clone())
                    .map_err(|error| input(format!("metadata.originalReport: {error}")))?,
            ),
            None => None,
        };
        let layout = self.artifact.session_layout(session)?;
        let token = hash_string_token(&layout.session.transaction_token);
        let wire = self
            .artifact
            .schema
            .wire
            .as_ref()
            .ok_or_else(|| input("wire schema required"))?;
        let positional_count = layout.report.positional_signals.len();
        let mut report = match original {
            Some(report) => {
                if report.positional_signals.len() != positional_count {
                    return Err(input("original report positional count mismatch"));
                }
                report
            }
            None => HeaderReport {
                errors: vec![],
                positional_signals: vec![None; positional_count],
                signals: vec![],
                keyed_signals: vec![],
                seeds: [0, 0],
                keysets: layout.keysets.clone(),
                header_context: None,
            },
        };
        let mut named = BTreeMap::new();
        for slot in &wire.slots {
            if let (Some(naming), WireRoute::Positional { positional }) =
                (&slot.naming, &slot.routing)
            {
                named.insert(naming.schema_key.clone(), (slot, *positional));
            }
        }
        for key in signals.keys() {
            if !named.contains_key(key) {
                return Err(input(format!("unknown signal {key}")));
            }
        }
        let mut expected = BTreeMap::new();
        for (name, (slot, positional)) in &named {
            match signals.get(name) {
                Some(Value::Object(fields)) => {
                    let original_raw = report.positional_signals[*positional]
                        .as_ref()
                        .map(|entry| entry.1.as_slice());
                    let raw = if self.artifact.direct && direct::is_local_storage(slot) {
                        direct::encode_local_storage(
                            slot,
                            fields,
                            original_raw,
                            token,
                            self.artifact.float_byte_order,
                        )
                    } else {
                        crate::signals::wire::named::encode_signal_source(
                            slot,
                            fields,
                            original_raw,
                            token,
                            self.artifact.float_byte_order,
                            Some(&self.number_policies),
                        )
                    }
                    .map_err(|error| input(format!("signal {name}: {error}")))?;
                    let extra = report.positional_signals[*positional]
                        .as_ref()
                        .map(|entry| entry.0.clone())
                        .unwrap_or_default();
                    report.positional_signals[*positional] = Some((extra, raw));
                    expected.insert(
                        name.clone(),
                        crate::signals::wire::named::canonical_fields(fields),
                    );
                }
                Some(_) => {
                    return Err(input(format!(
                        "the value of signal {name} must be an object of fields"
                    )))
                }
                None => report.positional_signals[*positional] = None,
            }
        }
        let headers = layout
            .generator()
            .map_err(input)?
            .encode_report_headers(&report)
            .map_err(input)?;
        let prefix = layout.header.prefix.clone();
        let entries = headers
            .iter()
            .map(|(name, value): (&str, &str)| {
                name.strip_prefix(&prefix)
                    .map(|suffix| (suffix.to_owned(), value.to_owned()))
                    .ok_or_else(|| {
                        input(format!("generated header {name:?} lacks the layout prefix"))
                    })
            })
            .collect::<Result<Vec<_>>>()?;
        let capture = CaptureHeaders {
            site_id: self.artifact.site_id.clone(),
            headers: Headers::from_entries(entries).map_err(input)?,
        };
        capture.validate()?;
        // The whole bundle must decode to exactly the requested signals: a
        // per-signal check cannot see ambiguity between emissions or a
        // channel/layout mismatch.
        let checked = self.decode(
            &capture,
            session,
            DecodeOptions {
                verify: false,
                metadata: false,
            },
        )?;
        let checked_fields = if self.artifact.direct {
            checked
                .positional_signals
                .iter()
                .map(|(name, fields)| (name.clone(), named::canonical_fields(fields)))
                .collect()
        } else {
            checked.positional_signals
        };
        if checked_fields != expected {
            return Err(input(
                "encoded capture does not decode to the requested signals",
            ));
        }
        Ok(capture)
    }

    /// Build the full metadata of one decoded signal on demand, from the raw
    /// bytes retained in `decoded.metadata.originalReport`. Pass the session the
    /// payload was decoded with. The result equals that signal's entry in a
    /// `metadata: true` decode with the same `verify` setting.
    pub fn explain(
        &self,
        decoded: &DecodedPayload,
        signal: &str,
        session: Option<&Session>,
        verify: bool,
    ) -> Result<NamedSignal> {
        let layout = self.artifact.session_layout(session)?;
        let token = hash_string_token(&layout.session.transaction_token);
        let wire = self
            .artifact
            .schema
            .wire
            .as_ref()
            .ok_or_else(|| input("wire schema required"))?;
        let (index, slot, positional) = wire
            .slots
            .iter()
            .enumerate()
            .find_map(|(index, slot)| match slot.routing {
                WireRoute::Positional { positional }
                    if slot.naming.as_ref().is_some_and(|n| n.schema_key == signal) =>
                {
                    Some((index, slot, positional))
                }
                _ => None,
            })
            .ok_or_else(|| input(format!("unknown signal {signal}")))?;
        let raw = match decoded
            .metadata
            .original_report
            .positional_signals
            .get(positional)
        {
            Some(Some((_, raw))) => raw,
            _ => {
                return Err(input(format!(
                    "signal {signal} is missing from this payload"
                )))
            }
        };
        let project = named::ProjectOptions {
            metadata: true,
            verify,
            constants: self.constants.get(index),
        };
        let (_, named, _) = self.artifact.decode_slot(
            slot,
            positional,
            raw,
            token,
            verify,
            project,
            &self.number_policies,
        )?;
        Ok(named)
    }
}

/// Per-call decode options.
///
/// Without `verify`, a slot with one usable recipe is decoded and projected
/// only. A slot with several recipes still re-encodes each candidate, because
/// exact re-encoding is what selects the recipe. With `verify`, every signal
/// must re-encode byte-identically and the rebuilt header bundle must equal the
/// input; `metadata.exactReencode` reports which mode ran.
///
/// Without `metadata`, only named fields are built and `metadata.signals` is
/// empty; [`ValidatedArtifact::explain`] builds one signal's metadata later from
/// the retained `metadata.originalReport`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DecodeOptions {
    pub verify: bool,
    pub metadata: bool,
}

/// Source compilation policy. The default preserves conservative compilation.
#[derive(Clone, Copy, Debug, Default)]
pub struct CompileOptions {
    pub direct: bool,
}

impl DecodeArtifact {
    /// Public layout descriptor; executable recipes remain in this artifact.
    pub fn compact_schema(&self) -> std::result::Result<CompactSeedSchema, WireError> {
        compact::export(self)
    }

    /// Analyze and compile once. The resulting JSON needs no source file on reuse.
    pub fn compile(
        source: &str,
        session: &Session,
        site_id: String,
        order: FloatByteOrder,
    ) -> Result<Self> {
        Self::compile_with_options(source, session, site_id, order, CompileOptions::default())
    }

    /// Compile explicit source-format fallbacks only when `options.direct` is set.
    pub fn compile_with_options(
        source: &str,
        session: &Session,
        site_id: String,
        order: FloatByteOrder,
        options: CompileOptions,
    ) -> Result<Self> {
        let pinned = pin(source, session).map_err(input)?;
        let schema = schema_from_source_with_options(source, options)?;
        let artifact = Self {
            format: ARTIFACT_FORMAT.into(),
            direct: options.direct,
            site_id,
            layout: pinned.layout,
            schema,
            float_byte_order: order,
        };
        artifact.validate()?;
        Ok(artifact)
    }

    pub fn from_json(bytes: &[u8]) -> Result<Self> {
        let artifact: Self = named::strict_json(bytes).map_err(input)?;
        artifact.validate()?;
        Ok(artifact)
    }

    pub fn validate(&self) -> Result<()> {
        if self.format != ARTIFACT_FORMAT {
            return Err(input("unsupported decode artifact format"));
        }
        if self.site_id.trim().is_empty() {
            return Err(input("artifact siteId must not be empty"));
        }
        // The redundant marker keeps accidental removal of the opt-in flag from
        // silently turning a direct artifact into a conservative artifact.
        let provenance = self.schema.diagnostics.get("directDecode");
        if self.direct != (provenance == Some(&Value::Bool(true)))
            || provenance.is_some_and(|value| value != &Value::Bool(true))
        {
            return Err(input("direct artifact provenance mismatch"));
        }
        self.layout.validate().map_err(input)?;

        let wire = self
            .schema
            .wire
            .as_ref()
            .ok_or_else(|| input("wire schema required"))?;
        if wire.version != WIRE_VERSION
            || wire.source != self.schema.source
            || self.layout.source.sha256 != wire.source.sha256
            || self.layout.source.bytes != wire.source.bytes
        {
            return Err(input("artifact source/schema identity mismatch"));
        }
        source_number::validate(&self.schema).map_err(input)?;
        let mut positions = BTreeSet::new();
        let mut names = BTreeSet::new();
        for slot in &wire.slots {
            if let Some(naming) = &slot.naming {
                semantic::validate(naming, &slot.contract).map_err(input)?;
            }
            if slot.programs.len() != slot.contract.emissions.len() {
                return Err(input("emission/program count mismatch"));
            }
            if let WireRoute::Positional { positional } = slot.routing {
                if positional >= self.layout.report.positional_signals.len()
                    || !positions.insert(positional)
                {
                    return Err(input("duplicate or out-of-range positional route"));
                }
                let descriptor = self.layout.report.positional_signals[positional].descriptor;
                if descriptor != slot.contract.descriptor as usize {
                    return Err(input("descriptor routing mismatch"));
                }
                if let Some(n) = &slot.naming {
                    if super::catalog::card(&n.canonical_name).is_none() {
                        return Err(input("unknown canonical signal identity"));
                    }
                    let descriptive = self
                        .schema
                        .signals
                        .get(&n.schema_key)
                        .ok_or_else(|| input("named slot missing from descriptive schema"))?;
                    if descriptive.address != slot.address || descriptive.routing != slot.routing {
                        return Err(input("named slot/schema routing mismatch"));
                    }
                    if n.schema_key.is_empty() || !names.insert(&n.schema_key) {
                        return Err(input("duplicate or empty signal name"));
                    }
                }
            }
        }
        if positions.len() != self.layout.report.positional_signals.len() {
            return Err(input("unmapped positional routes"));
        }
        Ok(())
    }

    /// Decode without the roundtrip verifier (see [`DecodeOptions`]).
    /// Load session overrides as a whole; never mix old token/keys with new session values.
    pub fn decode(
        &self,
        capture: &CaptureHeaders,
        session: Option<&Session>,
    ) -> Result<DecodedPayload> {
        self.decode_with(capture, session, DecodeOptions::default())
    }

    /// Decode with explicit options. `verify: true` re-encodes every signal and
    /// the whole header bundle and requires byte-identical results.
    pub fn decode_with(
        &self,
        capture: &CaptureHeaders,
        session: Option<&Session>,
        options: DecodeOptions,
    ) -> Result<DecodedPayload> {
        self.validate()?;
        let policies = source_number::validate(&self.schema).map_err(input)?;
        self.decode_validated(capture, session, options, None, &policies)
    }

    /// Callers guarantee `self.validate()` succeeded and nothing changed since.
    /// `constants` holds each wire slot's [`named::slot_constants`], by slot index.
    fn decode_validated(
        &self,
        capture: &CaptureHeaders,
        session: Option<&Session>,
        options: DecodeOptions,
        constants: Option<&[Map<String, Value>]>,
        number_policies: &source_number::Policies,
    ) -> Result<DecodedPayload> {
        let verify = options.verify;
        capture.validate()?;
        if self.site_id != capture.site_id {
            return Err(input("capture/artifact siteId mismatch"));
        }
        let layout = self.session_layout(session)?;
        let headers = capture.expanded(&layout.header.prefix)?;
        let original = layout.decode_headers(&headers).map_err(input)?;
        let report = original.report();
        let token = hash_string_token(&layout.session.transaction_token);
        let mut slim = BTreeMap::new();
        let mut detail = BTreeMap::new();
        let mut decoded = 0;
        let mut missing = 0;
        // Only the verifier rebuilds the report and header bundle.
        let mut rebuilt: Option<Report> = verify.then(|| report.clone());
        let wire = self
            .schema
            .wire
            .as_ref()
            .ok_or_else(|| input("wire schema required"))?;
        for (index, slot) in wire.slots.iter().enumerate() {
            let WireRoute::Positional { positional } = slot.routing else {
                continue;
            };
            // The explicit adapter retains unnamed/decoy entries in the report
            // without inventing named fields or counting them as named signals.
            if self.direct && slot.naming.is_none() {
                continue;
            }
            let Some((_, raw)) = &report.positional_signals[positional] else {
                missing += 1;
                continue;
            };
            let project = named::ProjectOptions {
                metadata: options.metadata,
                verify,
                constants: constants.and_then(|c| c.get(index)),
            };
            let (name, mut named, encoded) = self.decode_slot(
                slot,
                positional,
                raw,
                token,
                verify,
                project,
                number_policies,
            )?;
            let fields = if options.metadata {
                named.fields.clone()
            } else {
                named.fields.take()
            };
            slim.insert(
                name.to_owned(),
                fields.ok_or_else(|| DecodeError::Signal {
                    position: positional,
                    reason: "fields missing".into(),
                })?,
            );
            decoded += 1;
            if options.metadata {
                detail.insert(name.to_owned(), named);
            }
            if let (Some(rebuilt), Some(encoded)) = (&mut rebuilt, encoded) {
                if let Some(entry) = &mut rebuilt.positional_signals[positional] {
                    entry.1 = encoded;
                }
            }
        }
        if let Some(rebuilt) = rebuilt {
            if rebuilt != report {
                return Err(input("reconstructed report differs"));
            }
            let mut restored = original.clone();
            restored.positional_signals = rebuilt.positional_signals;
            let encoded_headers = layout
                .generator()
                .map_err(input)?
                .encode_report_headers(&restored)
                .map_err(input)?;
            if encoded_headers.iter().count() != headers.iter().count()
                || encoded_headers
                    .iter()
                    .any(|(name, value)| headers.get(name) != Some(value))
            {
                return Err(input("reconstructed headers differ"));
            }
        }
        Ok(DecodedPayload {
            site_id: self.site_id.clone(),
            errors: report.errors,
            positional_signals: slim,
            signals: report.signals,
            keyed_signals: report.keyed_signals,
            seeds: report.seeds,
            keysets: report.keysets,
            metadata: DecodeMetadata {
                original_report: original,
                decoded,
                signals: detail,
                missing,
                exact_reencode: verify,
                float_byte_order: self.float_byte_order,
            },
        })
    }

    /// The artifact layout with a whole replacement session, validated.
    fn session_layout(&self, session: Option<&Session>) -> Result<SeedLayout> {
        let mut layout = self.layout.clone();
        if let Some(session) = session {
            layout.session = session.clone();
        }
        layout.validate().map_err(input)?;
        Ok(layout)
    }

    /// Decode one present positional slot: select its recipe, then project.
    /// Returns the schema key, the projection and, when re-encoded, the bytes.
    fn decode_slot<'s>(
        &self,
        slot: &'s WireSlot,
        positional: usize,
        raw: &[u8],
        token: i32,
        verify: bool,
        project: named::ProjectOptions<'_>,
        number_policies: &source_number::Policies,
    ) -> Result<(&'s str, NamedSignal, Option<Vec<u8>>)> {
        let fail = |reason: String| DecodeError::Signal {
            position: positional,
            reason,
        };
        let name = slot
            .naming
            .as_ref()
            .ok_or_else(|| fail("signal has no canonical field mapping".into()))?;
        let seed = u32::from_be_bytes(
            raw.get(..4)
                .ok_or_else(|| fail("random prefix truncated".into()))?
                .try_into()
                .map_err(|_| fail("random prefix truncated".into()))?,
        );
        let mut accepted: Vec<(NamedSignal, Option<Vec<u8>>)> = vec![];
        let mut reasons = Vec::new();
        // Exact re-encoding selects among several recipes; with one recipe it
        // only verifies, so it runs only when asked.
        let usable = slot
            .programs
            .iter()
            .filter(|s| !matches!(s, Support::Unsupported { .. }))
            .count();
        let reencode = verify || usable > 1;
        for (emission, support) in slot.programs.iter().enumerate() {
            let program = match support {
                Support::Supported { program } | Support::RequiresInitializer { program, .. } => {
                    program
                }
                Support::Unsupported { reason, .. } => {
                    reasons.push(reason.clone());
                    continue;
                }
            };
            let attempt = (|| {
                let policy = number_policies.get(slot.contract.descriptor, emission);
                let source =
                    source_number::decode(program, raw, token, self.float_byte_order, policy)?;
                let value = source.value;
                let encoded = if reencode {
                    let encoded = source_number::encode(
                        program,
                        &value,
                        seed,
                        token,
                        self.float_byte_order,
                        policy,
                        policy.map(|_| &source.forms),
                    )?;
                    if encoded != raw {
                        return Err(WireError::Reencode);
                    }
                    Some(encoded)
                } else {
                    None
                };
                let root_case = if self.direct {
                    if let WireValue::Choice { arm, .. } = &value {
                        slot.source_plan
                            .get(emission)
                            .and_then(|plan| plan.root_cases.iter().find(|case| case.arm == *arm))
                            .cloned()
                    } else {
                        None
                    }
                } else {
                    None
                };
                let manual = if self.direct {
                    direct::manual_local_storage(&value, slot)
                } else {
                    None
                };
                let decoded = DecodedSlot {
                    positional,
                    descriptor: slot.contract.descriptor,
                    address: slot.address.clone(),
                    captured_seed: Some(seed),
                    raw: Some(raw.to_vec()),
                    outcome: Outcome::Decoded {
                        value,
                        exact_reencode: reencode,
                    },
                    selection: Some(Selection {
                        emissions: vec![emission],
                        proof: SelectionProof::IdenticalPrograms,
                        root_case,
                    }),
                };
                let named = manual.unwrap_or_else(|| {
                    named::project_source_with_options(
                        slot,
                        &decoded,
                        token,
                        &Ok(self.float_byte_order),
                        Some((emission, program)),
                        project,
                        policy.map(|p| (p, &source.forms)),
                    )
                });
                if named.status != NamedStatus::Decoded || named.fields.is_none() {
                    return Err(WireError::Type);
                }
                Ok((named, encoded))
            })();
            match attempt {
                Ok(candidate) => accepted.push(candidate),
                Err(error) => reasons.push(error.to_string()),
            }
        }
        let Some((first, _)) = accepted.first() else {
            return Err(fail(format!(
                "no supported exact decoding: {}",
                reasons.join("; ")
            )));
        };
        if accepted
            .iter()
            .any(|(other, _)| other.fields != first.fields)
        {
            return Err(fail("ambiguous decoded fields".into()));
        }
        let (named, encoded) = accepted.swap_remove(0);
        Ok((&name.schema_key, named, encoded))
    }
}

/// Shared source preparation for runtime artifacts and descriptive exports.
/// Unsupported recipes remain explicit; no runtime execution or session input.
fn schema_from_source_with_options(
    source: &str,
    options: CompileOptions,
) -> Result<SchemaDocument> {
    let analysis = analyze_program_signals(
        source,
        &SignalFinderOptions {
            layouts_only: true,
            finder_roles: true,
        },
    )
    .map_err(|e| input(format!("source analysis: {e}")))?;
    let schema_value = serde_json::to_value(&analysis.report).map_err(input)?;
    let mut schema: SchemaDocument = serde_json::from_value(schema_value.clone()).map_err(input)?;
    let audit = source_counts::audit(source, &schema_value).map_err(input)?;
    if options.direct {
        let diagnostics = schema
            .diagnostics
            .as_object_mut()
            .ok_or_else(|| input("schema diagnostics must be an object"))?;
        diagnostics.insert("directDecode".into(), Value::Bool(true));
    }

    let wire = schema
        .wire
        .as_mut()
        .ok_or_else(|| input("wire schema required"))?;
    let workers = source_counts::worker_inputs(source, &wire.slots).map_err(input)?;
    for slot in &mut wire.slots {
        for (index, emission) in slot.contract.emissions.iter().enumerate() {
            // Lifecycle readiness does not invalidate a proven wire recipe.
            // Only unsupported emissions need source-format fallback lowering.
            if matches!(
                slot.programs.get(index),
                Some(Support::Supported { .. } | Support::RequiresInitializer { .. })
            ) {
                continue;
            }
            let attempt = (|| {
                let salt = emission
                    .stream
                    .as_ref()
                    .and_then(|s| s.salt.as_deref())
                    .and_then(|salt| {
                        if options.direct {
                            direct::parse_salt(salt)
                        } else {
                            source_compiler::parse_salt(salt)
                        }
                    })
                    .ok_or_else(|| input("salt unresolved"))?;
                let context = direct::CompileContext {
                    direct: options.direct,
                    loops: Default::default(),
                    last_count: Default::default(),
                    counts: &audit.counts,
                    worker: workers
                        .iter()
                        .any(|w| w["emission"] == serde_json::json!(emission.site)),
                };
                let body = direct::compile(
                    &emission.grammar,
                    emission,
                    &wire.algorithms,
                    slot.source_plan
                        .get(index)
                        .ok_or_else(|| input("source plan missing"))?,
                    source,
                    vec![],
                    None,
                    &mut vec![],
                    &context,
                )
                .map_err(input)?;
                let program = Program {
                    prefix_byte_order: "be32".into(),
                    seed_source: "captured_prefix".into(),
                    stream_scope: "collector_local".into(),
                    string_scope: "collector_local".into(),
                    salt: salt as u32,
                    body,
                };
                Ok::<_, DecodeError>(program)
            })();

            let recipe = match attempt {
                Ok(program) => Support::Supported { program },
                Err(error) => Support::Unsupported {
                    reason: error.to_string(),
                    minimum_input: None,
                },
            };
            if let Some(existing) = slot.programs.get_mut(index) {
                *existing = recipe;
            } else {
                return Err(input("emission/program count mismatch"));
            }
        }
    }
    source_number::compile(&mut schema, source).map_err(input)?;
    Ok(schema)
}
