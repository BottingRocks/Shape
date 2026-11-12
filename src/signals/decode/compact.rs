//! Public encoding facts only. Session material and executable recipes stay external.
use super::DecodeArtifact;
use crate::signals::payload::extract::SeedSourceFacts;
use crate::signals::wire::{CompactSchema, FloatByteOrder, SchemaDocument, WireError, WireRoute};
use serde::Serialize;
use serde_json::{json, Value};

pub const COMPACT_SEED_SCHEMA_FORMAT: &str = "shape-seed-schema/1";

/// Descriptive seed export, deliberately not accepted by the artifact loader.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactSeedSchema {
    pub format: &'static str,
    /// Source-pinned literal emitted in the `-c` header.
    pub seed_id: String,
    pub encoding: Value,
    pub keysets: Vec<Vec<String>>,
    pub signals: CompactSchema,
    pub report_routes: Value,
    /// Logical contracts with no matching route in the pinned layout.
    pub unrouted_signals: Vec<String>,
}

impl CompactSeedSchema {
    /// Export source contracts without executing runtime recipes, parsing
    /// headers, or supplying alphabet, keys, token, or other session values.
    pub fn from_source(source: &str) -> Result<Self, super::DecodeError> {
        Self::from_source_with_options(source, super::CompileOptions::default())
    }

    /// Descriptive export using an explicitly selected source compilation policy.
    pub fn from_source_with_options(
        source: &str,
        options: super::CompileOptions,
    ) -> Result<Self, super::DecodeError> {
        let facts = crate::signals::payload::extract::pin_source(source).map_err(super::input)?;
        let schema = super::schema_from_source_with_options(source, options)?;
        export_facts(&schema, &facts, None)
            .map_err(|e| super::input(format!("compact export: {e}")))
    }
}

pub(super) fn export(a: &DecodeArtifact) -> Result<CompactSeedSchema, WireError> {
    export_facts(
        &a.schema,
        &SeedSourceFacts {
            source: a.layout.source.clone(),
            cipher: a.layout.cipher.clone(),
            keysets: a.layout.keysets.clone(),
            report: a.layout.report.clone(),
            header: a.layout.header.clone(),
        },
        Some(a.float_byte_order),
    )
}

fn export_facts(
    schema: &SchemaDocument,
    facts: &SeedSourceFacts,
    order: Option<FloatByteOrder>,
) -> Result<CompactSeedSchema, WireError> {
    let signals = schema.compact_schema()?;
    let mut header = serde_json::to_value(&facts.header).map_err(|_| WireError::Json)?;
    header["c"] = json!({"$ref":"#/seedId"});
    let mut associated = std::collections::BTreeSet::new();
    // A shared routing key alone does not prove descriptor identity for named
    // collectors. Their direct descriptor address must match as well.
    let mut contracts = |route: WireRoute, descriptor: usize| {
        let names: Vec<_> = schema
            .signals
            .iter()
            .filter_map(|(name, s)| {
                let direct = format!("direct:{descriptor}");
                (s.routing == route
                    && (matches!(route, WireRoute::Positional { .. }) || s.address == direct))
                    .then(|| name.clone())
            })
            .collect();
        associated.extend(names.iter().cloned());
        names
    };
    let positional: Vec<_> = facts
        .report
        .positional_signals
        .iter()
        .map(|r| {
            json!({
                "positionalSignal":r.slot, "descriptor":r.descriptor,
                "ctor":r.ctor, "priority":r.priority,
                "contracts":contracts(WireRoute::Positional { positional:r.slot }, r.descriptor)
            })
        })
        .collect();
    let named: Vec<_> = facts
        .report
        .signals
        .iter()
        .map(|r| {
            json!({
                "key":r.name, "descriptor":r.descriptor,
                "contracts":contracts(WireRoute::Name { name:json!(r.name) }, r.descriptor)
            })
        })
        .collect();
    let keyed: Vec<_> = facts
        .report
        .keyed_signals
        .iter()
        .map(|r| {
            json!({
                "key":r.name, "descriptor":r.descriptor,
                "contracts":contracts(WireRoute::Name { name:json!(r.name) }, r.descriptor)
            })
        })
        .collect();
    let number = schema
        .wire
        .as_ref()
        .and_then(|w| w.number_float.as_ref())
        .map(|f| {
            json!({
                "marker":f.marker, "width":f.width, "representation":f.representation,
                "nativeByteOrder":f.native_byte_order, "fallbackByteOrder":f.fallback_byte_order,
                "runtimePrerequisites":f.runtime_prerequisites
            })
        });
    Ok(CompactSeedSchema {
        format: COMPACT_SEED_SCHEMA_FORMAT,
        seed_id: facts.header.c.clone(),
        encoding: json!({
            "payloadFormat":crate::signals::payload::PROFILE_FORMAT,
            "cipher":facts.cipher,
            "header":header,
            "float":{"callerByteOrder":order,"number":number},
            "serializer":{
                "format":"superpack",
                "signals":{"keysets":"keysets","depthBound":100,"missingSentinelExtension":true},
                "errorsAndEntryValues":{"keysets":[],"depthBound":null,"missingSentinelExtension":false}
            },
            "entry":{"shape":["values","extra"],"values":"superpack array","extra":"raw bytes","missing":null,"missingWireBytes":[0,0]},
            "namedEntry":{"shape":["string key","raw value"],"value":"superpack value; missing sentinel extension supported"},
            "keyedEntry":{"shape":["u32 key","entry or null"]},
            "framing":{"lengths":"ULEB128","payloadSeedPrefix":"two big-endian u32 words","base64":"session alphabet; unpadded"},
            "externalInputs":{
                "session":["alphabet","keys","transactionToken","shuffleKey","nonce (when used)"],
                "message":["payload seeds (two u32 words)","collector seeds/entropy","observed values","optional header context overrides"]
            }
        }),
        keysets: facts.keysets.clone(),
        unrouted_signals: signals
            .0
            .keys()
            .filter(|n| !associated.contains(*n))
            .cloned()
            .collect(),
        signals,
        report_routes: json!({"errors":{"value":"superpack array"},"positionalSignals":positional,"signals":named,"keyedSignals":keyed}),
    })
}
