use super::{
    algorithms,
    program::{Support, WireValue},
    WireRoute, WireSchema, WIRE_VERSION,
};
use crate::signals::{
    codec::hash_string_token,
    extract::{contracts::signal_contracts_from_inputs, inputs::extract_program_inputs},
    payload::{Report, SeedLayout},
    schema::SourceIdentity,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Errors deliberately omit source text, session material and field values.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[serde(tag = "code", content = "detail", rename_all = "snake_case")]
pub enum WireError {
    #[error("invalid schema JSON")]
    Json,
    #[error("source identity mismatch")]
    Source,
    #[error("wire metadata does not match source evidence")]
    Evidence,
    #[error("unsupported wire schema version")]
    Version,
    #[error("schema/config descriptor routing mismatch")]
    Routing,
    #[error("invalid session/pin configuration")]
    Pin,
    #[error("report bounds or configuration mismatch")]
    Bounds,
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("truncated signal")]
    Truncated,
    #[error("invalid signal byte representation")]
    Codec,
    #[error("literal mismatch")]
    Literal,
    #[error("ambiguous source language membership")]
    Ambiguous,
    #[error("invalid discriminant")]
    Discriminant,
    #[error("value does not match wire type")]
    Type,
    #[error("unconsumed signal bytes: {0}")]
    Trailing(usize),
    #[error("whole-blob reencoding mismatch")]
    Reencode,
}
impl WireError {
    pub(super) fn codec(error: crate::signals::codec::CodecError) -> Self {
        match error {
            crate::signals::codec::CodecError::Truncated(_)
            | crate::signals::codec::CodecError::ShortSeedPrefix(_) => Self::Truncated,
            _ => Self::Codec,
        }
    }
}

/// Typed loader for the actual `shape signals … --json --layouts-only` file.
/// Old descriptive fields are retained verbatim and are never codec input.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SchemaDocument {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<String>,
    pub source: SourceIdentity,
    pub signals: BTreeMap<String, DescriptiveSignal>,
    pub diagnostics: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wire: Option<WireSchema>,
}
#[derive(Clone, Debug)]
pub struct DescriptiveSignal {
    pub address: String,
    pub routing: WireRoute,
    /// `type: "no_data"`: a declared-not-emitted slot listed as a signal with
    /// no fields (its routing stays in `name` / `positional`). It has no wire
    /// value, so decoders skip it; it is only validated against its slot.
    pub no_data: bool,
    pub description: BTreeMap<String, serde_json::Value>,
}
impl Serialize for DescriptiveSignal {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut object = serde_json::to_value(&self.routing).map_err(serde::ser::Error::custom)?;
        let map = object.as_object_mut().unwrap();
        map.insert("address".into(), self.address.clone().into());
        if self.no_data {
            map.insert("type".into(), "no_data".into());
        }
        for (key, value) in &self.description {
            if map.insert(key.clone(), value.clone()).is_some() {
                return Err(serde::ser::Error::custom("reserved descriptive member"));
            }
        }
        object.serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for DescriptiveSignal {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let mut entry = BTreeMap::<String, serde_json::Value>::deserialize(deserializer)?;
        let address = entry
            .remove("address")
            .and_then(|v| v.as_str().map(str::to_string))
            .ok_or_else(|| serde::de::Error::custom("missing address"))?;
        let kind = entry.remove("type");
        let positional = entry.remove("positional");
        let name = entry.remove("name");
        let kind = kind.as_ref().and_then(|v| v.as_str());
        let no_data = kind == Some("no_data");
        // A no-data entry keeps its routing member; the routing class follows
        // from which one it carries.
        let kind = match (kind, &positional, &name) {
            (Some("no_data"), Some(_), None) => Some("positional"),
            (Some("no_data"), None, Some(_)) => Some("name"),
            (kind, _, _) => kind,
        };
        let routing = match (kind, positional, name) {
            (Some("positional"), Some(positional), None) => WireRoute::Positional {
                positional: positional
                    .as_u64()
                    .and_then(|p| usize::try_from(p).ok())
                    .ok_or_else(|| serde::de::Error::custom("invalid position"))?,
            },
            (Some("name"), None, Some(name)) => WireRoute::Name { name },
            _ => return Err(serde::de::Error::custom("invalid signal routing")),
        };
        Ok(Self {
            address,
            routing,
            no_data,
            description: entry,
        })
    }
}
impl SchemaDocument {
    /// Stage session-bound witnesses into the SAME saved per-seed document.
    /// This parses the trusted initializer; it does not execute JavaScript.
    pub fn bind_initializer(
        &mut self,
        source: &str,
        pin: &SeedLayout,
        initializer: &str,
    ) -> Result<(), WireError> {
        let bytes = serde_json::to_vec(self).map_err(|_| WireError::Json)?;
        Decoder::load(&bytes, source, pin)?;
        let inputs = extract_program_inputs(source).map_err(|_| WireError::Evidence)?;
        let context = super::InitializationContext::from_initializer(
            initializer,
            &pin.session.transaction_token,
        )
        .map_err(|_| WireError::Pin)?;
        let schema = self.wire.as_mut().ok_or(WireError::Evidence)?;
        schema.initialization = Some(readiness_census(schema, source, &inputs, &context));
        Ok(())
    }

    pub fn from_json(bytes: &[u8]) -> Result<Self, WireError> {
        if bytes.len() > 128 * 1024 * 1024 {
            return Err(WireError::Bounds);
        }
        super::named::strict_json(bytes)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Outcome {
    Missing,
    Decoded {
        value: WireValue,
        exact_reencode: bool,
    },
    Unsupported {
        reason: WireError,
    },
    Invalid {
        reason: WireError,
    },
}
/// Raw bytes remain available even when semantic decoding is unsupported.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecodedSlot {
    pub positional: usize,
    pub descriptor: u32,
    pub address: String,
    pub captured_seed: Option<u32>,
    pub raw: Option<Vec<u8>>,
    pub outcome: Outcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection: Option<Selection>,
}
/// Source emission provenance plus the exact selected root-case certificate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    pub emissions: Vec<usize>,
    pub proof: SelectionProof,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_case: Option<super::RootCasePlan>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectionProof {
    IdenticalPrograms,
    EquivalentCountPrograms,
    UniqueCanonicalLanguage,
}

/// Validates saved schema against exact source and a verified session pin,
/// then executes only the saved, source-checked programs. No entropy is used.
/// Source analysis here is static; JavaScript is never executed.
pub struct Decoder {
    schema: WireSchema,
    programs: Vec<Vec<Support>>,
    token: i32,
    keysets: Vec<Vec<String>>,
    options: super::TypedDecoderOptions,
}
impl Decoder {
    pub fn load(schema_json: &[u8], source: &str, pin: &SeedLayout) -> Result<Self, WireError> {
        Self::load_with_options(
            schema_json,
            source,
            pin,
            super::TypedDecoderOptions::default(),
        )
    }
    /// Select collecting-runtime byte order externally; source/pin checks are unchanged.
    pub fn load_with_options(
        schema_json: &[u8],
        source: &str,
        pin: &SeedLayout,
        options: super::TypedDecoderOptions,
    ) -> Result<Self, WireError> {
        Self::load_context(schema_json, source, pin, options, None)
    }
    /// Enable only conditional programs whose per-instance witness revalidates
    /// against this trusted session initializer and the pinned token.
    pub fn load_with_initializer(
        schema_json: &[u8],
        source: &str,
        pin: &SeedLayout,
        initializer: &str,
    ) -> Result<Self, WireError> {
        Self::load_with_initializer_and_options(
            schema_json,
            source,
            pin,
            initializer,
            super::TypedDecoderOptions::default(),
        )
    }
    pub fn load_with_initializer_and_options(
        schema_json: &[u8],
        source: &str,
        pin: &SeedLayout,
        initializer: &str,
        options: super::TypedDecoderOptions,
    ) -> Result<Self, WireError> {
        let context = super::InitializationContext::from_initializer(
            initializer,
            &pin.session.transaction_token,
        )
        .map_err(|_| WireError::Pin)?;
        Self::load_context(schema_json, source, pin, options, Some(&context))
    }
    fn load_context(
        schema_json: &[u8],
        source: &str,
        pin: &SeedLayout,
        options: super::TypedDecoderOptions,
        context: Option<&super::InitializationContext>,
    ) -> Result<Self, WireError> {
        let document = SchemaDocument::from_json(schema_json)?;
        super::source_number::regenerate(&document, source)?;

        let mut schema = document.wire.ok_or_else(|| {
            WireError::Unsupported("schema has no versioned wire metadata".into())
        })?;
        if schema.version != WIRE_VERSION {
            return Err(WireError::Version);
        }
        let identity = SourceIdentity {
            sha256: algorithms::digest(source.as_bytes()),
            bytes: source.len(),
        };
        if identity != document.source
            || identity != schema.source
            || identity.sha256 != pin.source.sha256
            || identity.bytes != pin.source.bytes
        {
            return Err(WireError::Source);
        }
        pin.validate().map_err(|_| WireError::Pin)?;
        // The caller supplies verified initializer literals in the pin. Check
        // all source-derived pin facts as well, including filtering and slots.
        let derived =
            crate::signals::payload::pin(source, &pin.session).map_err(|_| WireError::Pin)?;
        if &derived.layout != pin {
            return Err(WireError::Pin);
        }
        let inputs = extract_program_inputs(source).map_err(|_| WireError::Evidence)?;
        let contracts =
            signal_contracts_from_inputs(source, &inputs).map_err(|_| WireError::Evidence)?;
        if schema.number_float.as_ref().is_some_and(|saved| {
            *saved != super::float::certify(source, &schema.algorithms.integer)
        }) {
            return Err(WireError::Evidence);
        }
        if schema.encoding != inputs.encoding
            || schema.algorithms != algorithms::certify(source, &inputs.encoding, &inputs.wiring)
            || schema.slots.len() != contracts.signals.len()
        {
            return Err(WireError::Evidence);
        }
        for (slot, contract) in schema.slots.iter().zip(&contracts.signals) {
            if &slot.contract != contract {
                return Err(WireError::Evidence);
            }
            let programs = super::programs_for(contract, &schema.algorithms, source);
            if slot.source_plan
                != contract
                    .emissions
                    .iter()
                    .map(|e| super::control::source_plan(source, e, &schema.algorithms))
                    .collect::<Vec<_>>()
            {
                return Err(WireError::Evidence);
            }
            if slot.programs != programs {
                return Err(WireError::Evidence);
            }
        }
        // Regenerate semantic bindings from exact source. Neither descriptions
        // nor caller-supplied display names are accepted as binding evidence.
        if schema.slots.iter().any(|slot| slot.naming.is_some()) {
            let expected = crate::signals::analysis::analyze_program_signals(
                source,
                &crate::signals::analysis::SignalFinderOptions::default(),
            )
            .map_err(|_| WireError::Evidence)?;
            if schema.slots.len() != expected.report.wire.slots.len() {
                return Err(WireError::Evidence);
            }
            for (saved, actual) in schema.slots.iter().zip(&expected.report.wire.slots) {
                if saved.naming != actual.naming {
                    return Err(WireError::Evidence);
                }
            }
            if serde_json::to_value(&document.signals).map_err(|_| WireError::Json)?
                != serde_json::to_value(&expected.report.signals).map_err(|_| WireError::Json)?
            {
                return Err(WireError::Evidence);
            }
        }
        if let Some(context) = context {
            let actual = readiness_census(&schema, source, &inputs, context);
            if let Some(saved) = &schema.initialization {
                if saved != &actual {
                    return Err(WireError::Evidence);
                }
                for entry in &saved.emissions {
                    if let super::InstanceReadiness::Ready { witness } = &entry.outcome {
                        let slot = schema
                            .slots
                            .iter()
                            .find(|slot| slot.contract.descriptor == entry.descriptor)
                            .ok_or(WireError::Evidence)?;
                        let emission = slot
                            .contract
                            .emissions
                            .get(entry.emission)
                            .ok_or(WireError::Evidence)?;
                        let concrete = match entry.root_case {
                            Some(arm) => super::control::root_emissions(source, emission)
                                .and_then(|cases| {
                                    cases
                                        .into_iter()
                                        .find(|(plan, _)| plan.arm == arm)
                                        .map(|(_, emission)| emission)
                                })
                                .ok_or(WireError::Evidence)?,
                            None => emission.clone(),
                        };
                        schema
                            .algorithms
                            .verify_stream_readiness(
                                witness,
                                source,
                                &inputs.encoding,
                                &inputs.wiring,
                                &concrete,
                                context,
                            )
                            .map_err(|_| WireError::Evidence)?;
                    }
                }
            }
            // Source-only documents may be staged directly at load. In either
            // path every result is regenerated from the exact source/session.
            for slot in &mut schema.slots {
                for (emission, support) in slot.programs.iter_mut().enumerate() {
                    let Support::RequiresInitializer {
                        program,
                        stream_body,
                        ..
                    } = support
                    else {
                        continue;
                    };
                    let mut program = program.clone();
                    if let super::Op::RootChoice { cases } = &mut program.body {
                        for case in cases {
                            if !matches!(case.support.as_ref(), Support::Supported { .. }) {
                                continue;
                            }
                            let entry = actual
                                .emissions
                                .iter()
                                .find(|entry| {
                                    entry.descriptor == slot.contract.descriptor
                                        && entry.emission == emission
                                        && entry.root_case == Some(case.plan.arm)
                                })
                                .ok_or(WireError::Evidence)?;
                            match &entry.outcome {
                                super::InstanceReadiness::Ready { witness } => {
                                    if &witness.body != stream_body {
                                        return Err(WireError::Evidence);
                                    }
                                }
                                super::InstanceReadiness::Refused { reason } => {
                                    case.support = Box::new(Support::Unsupported {
                                        reason: format!(
                                            "instance readiness proof refused: {reason}"
                                        ),
                                        minimum_input: None,
                                    })
                                }
                            }
                        }
                        // The wrapper performs no stateful operation; selected
                        // children retain their own readiness refusal.
                        *support = Support::Supported { program };
                    } else if let Some(super::EmissionReadiness {
                        outcome: super::InstanceReadiness::Ready { witness },
                        ..
                    }) = actual.emissions.iter().find(|entry| {
                        entry.descriptor == slot.contract.descriptor
                            && entry.emission == emission
                            && entry.root_case.is_none()
                    }) {
                        if &witness.body != stream_body {
                            return Err(WireError::Evidence);
                        }
                        *support = Support::Supported { program };
                    }
                }
            }
            schema.initialization = Some(actual);
        } else if schema.initialization.is_some() {
            return Err(WireError::Unsupported(
                "saved initialization witnesses require the trusted initializer".into(),
            ));
        }
        Ok(Self::validated(document.signals, schema, pin)?.with_options(options))
    }
    /// Applying options never upgrades an unproven source primitive.
    pub fn with_options(mut self, options: super::TypedDecoderOptions) -> Self {
        self.options = options;
        self
    }
    fn float_format(&self) -> Result<super::FloatByteOrder, WireError> {
        self.schema
            .number_float
            .as_ref()
            .ok_or_else(|| {
                WireError::Unsupported(
                    "source float metadata is absent; native dependency proof required".into(),
                )
            })?
            .format(self.options)
    }
    fn validated(
        descriptions: BTreeMap<String, DescriptiveSignal>,
        schema: WireSchema,
        pin: &SeedLayout,
    ) -> Result<Self, WireError> {
        let mut positions = vec![None; pin.report.positional_signals.len()];
        let mut descriptors = BTreeSet::new();
        for (index, slot) in schema.slots.iter().enumerate() {
            use crate::signals::extract::contracts::ProducerKind;
            let address = match slot.contract.producer.kind {
                ProducerKind::Grouped {
                    group, position, ..
                } => format!("wave:{group}:{position}"),
                ProducerKind::Direct { .. } => format!("direct:{}", slot.contract.descriptor),
                ProducerKind::Unrouted => format!("unrouted:{}", slot.contract.descriptor),
            };
            if slot.address != address {
                return Err(WireError::Routing);
            }
            if !descriptors.insert(slot.contract.descriptor) {
                return Err(WireError::Routing);
            }
            match &slot.routing {
                WireRoute::Positional { positional } => {
                    let p = pin
                        .report
                        .positional_signals
                        .get(*positional)
                        .ok_or(WireError::Routing)?;
                    if p.descriptor != slot.contract.descriptor as usize
                        || positions[*positional].replace(index).is_some()
                    {
                        return Err(WireError::Routing);
                    }
                }
                WireRoute::Name { name } => {
                    if !(pin.report.signals.iter().any(|s| {
                        s.descriptor == slot.contract.descriptor as usize
                            && name.as_str() == Some(s.name.as_str())
                    }) || pin.report.keyed_signals.iter().any(|s| {
                        s.descriptor == slot.contract.descriptor as usize
                            && name.as_u64() == Some(s.name as u64)
                    })) {
                        return Err(WireError::Routing);
                    }
                }
            }
        }
        if positions.iter().any(Option::is_none) {
            return Err(WireError::Routing);
        }
        let mut seen = BTreeSet::new();
        for description in descriptions.values() {
            let Some(slot) = schema
                .slots
                .iter()
                .find(|s| s.address == description.address && s.routing == description.routing)
            else {
                return Err(WireError::Routing);
            };
            if !seen.insert(slot.contract.descriptor) {
                return Err(WireError::Routing);
            }
        }
        let programs = positions
            .into_iter()
            .map(|i| {
                let slot = &schema.slots[i.unwrap()];
slot.programs.iter().enumerate().map(|(emission,support)| match support {
    Support::RequiresInitializer { .. } => Support::Unsupported {
        reason: schema.initialization.as_ref().and_then(|census|census.emissions.iter().find(|entry|entry.descriptor==slot.contract.descriptor && entry.emission==emission)).and_then(|entry|match &entry.outcome {super::InstanceReadiness::Refused{reason}=>Some(format!("instance readiness proof refused: {reason}")),_=>None}).unwrap_or_else(||"this emission requires a trusted session initializer and readiness witness".into()),
        minimum_input:None,
    },
    other=>other.clone(),
}).collect()
            })
            .collect();
        Ok(Self {
            schema,
            programs,
            token: hash_string_token(&pin.session.transaction_token),
            keysets: pin.keysets.clone(),
            options: super::TypedDecoderOptions::default(),
        })
    }
    pub fn decode_report(&self, report: &Report) -> Result<Vec<DecodedSlot>, WireError> {
        if report.positional_signals.len() != self.programs.len() || report.keysets != self.keysets
        {
            return Err(WireError::Bounds);
        }
        report.to_payload().map_err(|_| WireError::Bounds)?;
        report
            .positional_signals
            .iter()
            .enumerate()
            .map(|(positional, entry)| {
                let slot = self
                    .schema
                    .slots
                    .iter()
                    .find(|s| s.routing == WireRoute::Positional { positional })
                    .ok_or(WireError::Routing)?;
                let raw = entry.as_ref().map(|(_, b)| b.clone());
                let captured_seed = raw
                    .as_ref()
                    .and_then(|b| crate::signals::codec::signal_seed(b).ok());
                let mut selection = None;
                let outcome = match entry {
                    None => Outcome::Missing,
                    Some((generic, _)) if !generic.is_empty() => Outcome::Unsupported {
                        reason: WireError::Unsupported(
                            "generic value channel is not a signal byte program".into(),
                        ),
                    },
                    Some((_, bytes)) => {
                        let result = select_with_float(
                            &self.programs[positional],
                            bytes,
                            self.token,
                            &self.float_format(),
                        )
                        .map(|(value, selected)| {
                            selection = Some(selected);
                            value
                        });
                        match result {
                            Ok(value) => Outcome::Decoded {
                                value,
                                exact_reencode: true,
                            },
                            Err(
                                reason @ (WireError::Unsupported(_)
                                | WireError::Ambiguous
                                | WireError::Bounds),
                            ) => Outcome::Unsupported { reason },
                            Err(reason) => Outcome::Invalid { reason },
                        }
                    }
                };
                Ok(DecodedSlot {
                    positional,
                    descriptor: slot.contract.descriptor,
                    address: slot.address.clone(),
                    captured_seed,
                    raw,
                    outcome,
                    selection,
                })
            })
            .collect()
    }
    /// Source-proven names plus complete, explicit original-report metadata.
    pub fn decode_named_report(&self, report: &Report) -> Result<super::NamedDocument, WireError> {
        let decoded = self.decode_report(report)?;
        super::named::document(
            &self.schema,
            &decoded,
            report,
            self.token,
            self.options,
            &self.float_format(),
        )
    }
    /// Reencode a supported value with the caller's captured seed.
    pub fn encode_slot(
        &self,
        positional: usize,
        value: &WireValue,
        captured_seed: u32,
    ) -> Result<Vec<u8>, WireError> {
        let programs = self.programs.get(positional).ok_or(WireError::Bounds)?;
        let first = programs
            .first()
            .ok_or_else(|| WireError::Unsupported("no emission grammar".into()))?;
        if programs.iter().any(|p| !same_execution(p, first)) {
            return Err(WireError::Unsupported(
                "encoding requires explicit emission selection".into(),
            ));
        }
        self.encode_slot_for_emission(positional, 0, value, captured_seed)
    }
    /// Explicit source emission selection retains that message's salt.
    pub fn encode_slot_for_emission(
        &self,
        positional: usize,
        emission: usize,
        value: &WireValue,
        captured_seed: u32,
    ) -> Result<Vec<u8>, WireError> {
        match self
            .programs
            .get(positional)
            .and_then(|p| p.get(emission))
            .ok_or(WireError::Bounds)?
        {
            Support::Supported { program } => {
                program.encode_with_float(value, captured_seed, self.token, &self.float_format())
            }
            Support::Unsupported { reason, .. } => Err(WireError::Unsupported(reason.clone())),
            Support::RequiresInitializer { .. } => Err(WireError::Unsupported(
                "selected emission requires a trusted initializer".into(),
            )),
        }
    }
}

/// Every explicit source candidate must be a canonical full match or proven
/// invalid. UNKNOWN, including unsupported float paths, blocks a winner.
#[cfg(test)]
fn select(
    programs: &[Support],
    bytes: &[u8],
    token: i32,
) -> Result<(WireValue, Selection), WireError> {
    select_with_float(
        programs,
        bytes,
        token,
        &Err(WireError::Unsupported(
            "float runtime byte order is unknown".into(),
        )),
    )
}
fn select_with_float(
    programs: &[Support],
    bytes: &[u8],
    token: i32,
    format: &Result<super::FloatByteOrder, WireError>,
) -> Result<(WireValue, Selection), WireError> {
    let Some(first) = programs.first() else {
        return Err(WireError::Unsupported("no emission grammar".into()));
    };
    let identical_source = programs.iter().all(|p| p == first);
    let identical = programs.iter().all(|p| same_execution(p, first));
    // An unproven candidate has no certified length language. Even a short
    // input cannot turn it into INVALID merely because a known case has RAW4.
    if let Some(reason) = programs.iter().find_map(|p| match p {
        Support::Unsupported { reason, .. } => Some(reason),
        _ => None,
    }) {
        return Err(WireError::Unsupported(reason.clone()));
    }
    if programs
        .iter()
        .any(|support| matches!(support, Support::RequiresInitializer { .. }))
    {
        return Err(WireError::Unsupported(
            "candidate instance readiness is unproven".into(),
        ));
    }
    let seed = crate::signals::codec::signal_seed(bytes).map_err(WireError::codec)?;
    let mut valid = Vec::new();
    let mut unknown = None;
    let mut invalid = WireError::Discriminant;
    for (emission, support) in programs.iter().enumerate() {
        if identical && emission > 0 {
            break;
        }
        let program = match support {
            Support::Supported { program } => program,
            Support::RequiresInitializer { .. } => {
                unknown = Some(WireError::Unsupported(
                    "candidate instance readiness is unproven".into(),
                ));
                continue;
            }
            Support::Unsupported { reason, .. } => {
                unknown = Some(WireError::Unsupported(reason.clone()));
                continue;
            }
        };
        let (min, max) = program.length_bounds();
        if !identical && (bytes.len() < min || max.is_some_and(|m| bytes.len() > m)) {
            invalid = WireError::Discriminant;
            continue;
        }
        let result = program
            .decode_with_float(bytes, token, format)
            .and_then(|value| {
                if program.encode_with_float(&value, seed, token, format)? != bytes {
                    return Err(WireError::Reencode);
                }
                Ok(value)
            });
        match result {
            Ok(value) => valid.push((emission, value)),
            Err(e @ (WireError::Unsupported(_) | WireError::Bounds | WireError::Ambiguous)) => {
                unknown = Some(e)
            }
            Err(e) => invalid = e,
        }
    }
    if let Some(e) = unknown {
        return Err(e);
    }
    if valid.len() > 1 {
        return Err(WireError::Ambiguous);
    }
    let (emission, value) = valid.pop().ok_or(invalid)?;
    let root_case = match (&programs[emission], &value) {
        (Support::Supported { program }, WireValue::Choice { arm, .. }) => match &program.body {
            super::Op::RootChoice { cases } => cases
                .iter()
                .find(|c| c.plan.arm == *arm)
                .map(|c| c.plan.clone()),
            _ => None,
        },
        _ => None,
    };
    Ok((
        value,
        Selection {
            root_case,
            emissions: if identical {
                (0..programs.len()).collect()
            } else {
                vec![emission]
            },
            proof: if identical_source {
                SelectionProof::IdenticalPrograms
            } else if identical {
                SelectionProof::EquivalentCountPrograms
            } else {
                SelectionProof::UniqueCanonicalLanguage
            },
        },
    ))
}

fn same_execution(a: &Support, b: &Support) -> bool {
    match (a, b) {
        (Support::Supported { program: a }, Support::Supported { program: b }) => {
            a.same_execution(b)
        }
        _ => a == b,
    }
}

#[cfg(test)]
#[path = "consumer_tests.rs"]
mod tests;

fn readiness_census(
    schema: &WireSchema,
    source: &str,
    inputs: &crate::signals::extract::inputs::ProgramInputReport,
    context: &super::InitializationContext,
) -> super::InitializationCensus {
    let mut emissions = Vec::new();
    for slot in &schema.slots {
        for (emission, contract) in slot.contract.emissions.iter().enumerate() {
            let candidates = super::control::root_emissions(source, contract)
                .map(|cases| {
                    cases
                        .into_iter()
                        .map(|(plan, contract)| (Some(plan.arm), contract))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_else(|| vec![(None, contract.clone())]);
            for (root_case, contract) in candidates {
                let outcome = match schema.algorithms.certify_stream_instance(
                    source,
                    &inputs.encoding,
                    &inputs.wiring,
                    &contract,
                    context,
                ) {
                    Ok(witness) => super::InstanceReadiness::Ready { witness },
                    Err(error) => super::InstanceReadiness::Refused {
                        reason: error.to_string(),
                    },
                };
                emissions.push(super::EmissionReadiness {
                    descriptor: slot.contract.descriptor,
                    emission,
                    root_case,
                    outcome,
                });
            }
        }
    }
    super::InitializationCensus {
        initializer_sha256: context.initializer_sha256().to_string(),
        required_context:
            super::InitializationEpoch::SuppliedPublicInitializerWithoutInterveningInitialization,
        emissions,
    }
}
