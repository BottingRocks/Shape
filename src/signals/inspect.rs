//! Capture-backed card observations. Sizes describe retained signal blobs, never JSON.
use super::{
    catalog,
    decode::{CaptureHeaders, DecodeMetadata, DecodedPayload, ValidatedArtifact},
    payload::{extract::Session, superpack::Value as Raw},
    wire::{named::strict_json, Support, WireRoute},
};
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// Inspect a saved artifact and capture without source or illustrative values.
///
/// Positional sizes include the captured four-byte random prefix, but exclude
/// outer container framing. Generic named values share a superpack channel;
/// without independent source spans their sizes are deliberately unavailable.
/// Multiple routes for a canonical signal produce an array of observations and
/// an aggregate size only when every observation has an independent exact span.
pub fn inspect(
    artifact: &[u8],
    headers: &[u8],
    session: Option<&[u8]>,
) -> Result<BTreeMap<String, Value>, String> {
    let validated = ValidatedArtifact::from_json(artifact).map_err(|e| e.to_string())?;
    let capture: CaptureHeaders = strict_json(headers).map_err(|e| e.to_string())?;
    let session: Option<Session> = session
        .map(strict_json)
        .transpose()
        .map_err(|e| e.to_string())?;
    capture.validate().map_err(|e| e.to_string())?;
    let artifact = validated.artifact();
    if capture.site_id != artifact.site_id {
        return Err("capture/artifact siteId mismatch".into());
    }
    let mut layout = artifact.layout.clone();
    if let Some(session) = &session {
        layout.session = session.clone();
    }
    layout.validate().map_err(|e| e.to_string())?;
    let original = layout
        .decode_headers(
            &capture
                .expanded(&layout.header.prefix)
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
    // Explain is the safe per-slot decoder. Retaining the envelope separately
    // allows unsupported slots to coexist with useful supported observations.
    let decoded = DecodedPayload {
        site_id: artifact.site_id.clone(),
        errors: vec![],
        positional_signals: BTreeMap::new(),
        signals: vec![],
        keyed_signals: vec![],
        seeds: original.seeds,
        keysets: original.keysets.clone(),
        metadata: DecodeMetadata {
            original_report: original,
            signals: BTreeMap::new(),
            decoded: 0,
            missing: 0,
            exact_reencode: false,
            float_byte_order: artifact.float_byte_order,
        },
    };
    let report = &decoded.metadata.original_report;
    let mut observations: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    let wire = artifact
        .schema
        .wire
        .as_ref()
        .ok_or("wire schema required")?;
    for (key, description) in &artifact.schema.signals {
        let slot = wire.slots.iter().find(|slot| {
            slot.address == description.address && slot.routing == description.routing
        });
        let canonical = slot
            .and_then(|s| s.naming.as_ref())
            .map(|n| n.canonical_name.as_str())
            .or_else(|| canonical_key(key));
        let Some(canonical) = canonical.filter(|name| catalog::card(name).is_some()) else {
            continue;
        };
        let observation = if description.no_data {
            entry(
                "no_data",
                Value::Null,
                None,
                "Build declares this route without a wire value",
            )
        } else {
            match &description.routing {
                WireRoute::Positional { positional } => {
                    let raw = report
                        .positional_signals
                        .get(*positional)
                        .ok_or("descriptive positional route is out of range")?;
                    match raw {
                        None => entry(
                            "missing",
                            Value::Null,
                            None,
                            "Route exists in build; capture contains the missing-entry sentinel",
                        ),
                        Some((generic, _)) if !generic.is_empty() => entry(
                            "unavailable",
                            Value::Null,
                            None,
                            "Positional entry includes generic packed values; its raw extra alone is not a complete encoded signal blob",
                        ),
                        Some((_, raw)) => {
                            let bytes = raw.len() as u64;
                            let supported = slot.is_some_and(|s| {
                                s.naming.as_ref().is_some_and(|n| n.mapping_error.is_none())
                                    && s.programs
                                        .iter()
                                        .any(|p| !matches!(p, Support::Unsupported { .. }))
                            });
                            if supported {
                                let name = &slot.unwrap().naming.as_ref().unwrap().schema_key;
                                let signal = validated
                                    .explain(&decoded, name, session.as_ref(), false)
                                    .map_err(|e| e.to_string())?;
                                entry("captured", signal.fields.map(Value::Object).unwrap_or(Value::Null), Some(bytes), "Captured positional raw blob, including its 4-byte random prefix; excludes container framing")
                            } else {
                                let mut value = entry("unavailable", Value::Null, Some(bytes), "Captured positional raw blob; full retained span including any prefix, excludes container framing");
                                value["note"] = json!("Route is present, but this artifact has no supported logical field decoder");
                                value
                            }
                        }
                    }
                }
                WireRoute::Name { name } => {
                    if let Some(name) = name.as_str() {
                        let routed = layout.report.signals.iter().any(|route| route.name == name);
                        if !routed {
                            entry(
                                "unavailable",
                                Value::Null,
                                None,
                                "Named route is absent from the artifact report layout",
                            )
                        } else {
                            let values: Vec<_> = report
                                .signals
                                .iter()
                                .filter_map(|raw| named_value(raw, name, canonical))
                                .filter(|v| !matches!(v, Raw::MissingSentinel))
                                .collect();
                            named_values(canonical, &values)
                        }
                    } else if let Some(name) = name.as_u64().and_then(|n| u32::try_from(n).ok()) {
                        let entries: Vec<_> = report
                            .keyed_signals
                            .iter()
                            .filter(|(n, _)| *n == name)
                            .filter_map(|(_, e)| e.as_ref())
                            .collect();
                        if !layout.report.keyed_signals.iter().any(|r| r.name == name) {
                            entry(
                                "unavailable",
                                Value::Null,
                                None,
                                "Numeric route is absent from the artifact report layout",
                            )
                        } else if entries.is_empty() {
                            entry(
                                "missing",
                                Value::Null,
                                None,
                                "Numeric route exists in build; no value in capture",
                            )
                        } else {
                            let values: Vec<_> = entries.iter().flat_map(|e| e.0.iter()).collect();
                            let mut value = named_values(canonical, &values);
                            if entries.iter().any(|e| !e.1.is_empty()) {
                                value = entry("unavailable", Value::Null, None, "Numeric route contains raw extras without a supported logical decoder");
                            }
                            value
                        }
                    } else {
                        entry(
                            "unavailable",
                            Value::Null,
                            None,
                            "Named route cannot be resolved to a string or u32 key",
                        )
                    }
                }
            }
        };
        observations
            .entry(canonical.into())
            .or_default()
            .push(observation);
    }
    Ok(catalog::cards()
        .map(|card| {
            let value = observations
                .remove(card.signal)
                .map(combine)
                .unwrap_or_else(|| {
                    entry(
                        "unavailable",
                        Value::Null,
                        None,
                        "Signal has no recognized route in this build",
                    )
                });
            (card.signal.into(), value)
        })
        .collect())
}

fn canonical_key(key: &str) -> Option<&str> {
    if catalog::card(key).is_some() {
        return Some(key);
    }
    let (name, suffix) = key.rsplit_once('_')?;
    (suffix.parse::<usize>().ok()? >= 2 && catalog::card(name).is_some()).then_some(name)
}

// The snapshot partitions string-named descriptors as [descriptor.name,
// bundle]. includeDebugNames wraps the bundle in {signal, value}; the outer
// route key remains authoritative, since the debug signal can be a descriptor.
fn named_value<'a>(raw: &'a Raw, name: &str, canonical: &str) -> Option<&'a Raw> {
    let Raw::Array(pair) = raw else { return None };
    let [Raw::String(key), value] = pair.as_slice() else {
        return None;
    };
    if key != name {
        return None;
    }
    if let Raw::Object(fields) = value {
        // Field names alone cannot certify a debug envelope: applications may
        // emit logical objects containing both `signal` and `value`.
        let debug_identity = fields.iter().any(|(key, signal)| {
            key == "signal"
                && match signal {
                    Raw::String(identity) => identity == name || identity == canonical,
                    Raw::Object(descriptor) => descriptor.iter().any(|(key, value)| {
                        key == "name" && matches!(value, Raw::String(identity) if identity == name)
                    }),
                    _ => false,
                }
        });
        if fields.len() == 2 && debug_identity {
            return fields.iter().find(|(k, _)| k == "value").map(|(_, v)| v);
        }
    }
    Some(value)
}

fn entry(status: &str, data: Value, bytes: Option<u64>, basis: &str) -> Value {
    json!({"status":status,"data":data,"size":{"bytes":bytes,"kind":if bytes.is_some(){"exact"}else{"unavailable"},"basis":basis}})
}

fn named_values(canonical: &str, values: &[&Raw]) -> Value {
    if values.is_empty() {
        return entry(
            "missing",
            Value::Null,
            None,
            "Named route exists in build; no emitted value in capture",
        );
    }
    let data: Vec<_> = values
        .iter()
        .map(|v| {
            let data = logical(v);
            if matches!(v, Raw::Object(_) | Raw::Array(_)) {
                return data;
            }
            // Bare named values have no wire object key. Use the approved single
            // logical field name where available, without borrowing example data.
            let field = catalog::card(canonical)
                .and_then(|card| match card.body {
                    catalog::card::CardBody::Fields([field]) => Some(field.name),
                    _ => None,
                })
                .unwrap_or("value");
            let mut fields = serde_json::Map::new();
            fields.insert(field.into(), data);
            Value::Object(fields)
        })
        .collect();
    let mut value = entry(
        "captured",
        if data.len() == 1 {
            data[0].clone()
        } else {
            Value::Array(data)
        },
        None,
        "Shared superpack channel has no independent captured signal blob span",
    );
    if values.len() > 1 {
        value["note"] = json!(format!(
            "{} emitted values retained in capture order",
            values.len()
        ));
    }
    value
}

fn logical(value: &Raw) -> Value {
    match value {
        Raw::Null => Value::Null,
        Raw::Undefined => json!({"$special":"undefined"}),
        Raw::MissingSentinel => json!({"$special":"missing"}),
        Raw::Bool(v) => json!(v),
        Raw::Number(v) => {
            let n = v.as_f64();
            if n.is_nan() {
                json!({"$special":"NaN"})
            } else if n.is_infinite() {
                json!({"$special": if n.is_sign_negative() { "-Infinity" } else { "Infinity" }})
            } else if n == 0.0 && n.is_sign_negative() {
                json!({"$special":"-0"})
            } else {
                json!(n)
            }
        }
        Raw::String(v) => json!(v),
        Raw::Array(v) => Value::Array(v.iter().map(logical).collect()),
        Raw::Object(v) => Value::Object(v.iter().map(|(k, v)| (k.clone(), logical(v))).collect()),
        Raw::Date(v) => json!({"milliseconds":v}),
        // Opaque wire bytes are not logical field data.
        Raw::Binary(_) => json!({"$special":"binary"}),
        Raw::Extension { .. } => json!({"$special":"extension"}),
    }
}

fn combine(mut entries: Vec<Value>) -> Value {
    if entries.len() == 1 {
        return entries.remove(0);
    }
    let total = entries
        .iter()
        .try_fold(0u64, |sum, e| sum.checked_add(e["size"]["bytes"].as_u64()?));
    let status = if entries.iter().any(|e| e["status"] == "captured") {
        "captured"
    } else if entries.iter().all(|e| e["status"] == "no_data") {
        "no_data"
    } else if entries.iter().all(|e| e["status"] == "missing") {
        "missing"
    } else {
        "unavailable"
    };
    let mut value = entry(
        status,
        Value::Array(entries.iter().map(|e| e["data"].clone()).collect()),
        total,
        if total.is_some() {
            "Sum of independent captured raw blob spans, including prefixes; excludes container framing"
        } else {
            "Multiple build routes; not every observation has an independent captured blob span"
        },
    );
    value["note"] = json!(format!(
        "{} build routes retained in schema-key order; statuses: {}",
        entries.len(),
        entries
            .iter()
            .map(|e| e["status"].as_str().unwrap_or("unavailable"))
            .collect::<Vec<_>>()
            .join(", ")
    ));
    value
}
