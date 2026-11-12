//! Generic source report ↔ encrypted, finalized custom-base64 payload.
//! No collectors, signal layouts, header context or entropy acquisition.
use serde::{Deserialize, Serialize};

use super::superpack::{pack, unpack_canonical, Limits, SerializerOptions, Value};
use super::{
    check_size, invalid, uleb, Container, Data, Error, KeyedEntry, Payload, PayloadCodec,
    PositionalEntry, MAX_CONTAINER_ENTRIES,
};

/// Source entry pair: generic values, then already-produced raw extra bytes.
/// `None` at an entry position means the source missing-entry sentinel.
pub type Entry = (Vec<Value>, Vec<u8>);

/// The four source fields stay at the root. Seeds and the ordered omitted
/// keysets are explicit metadata, so decoded reports can be encoded directly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Report {
    pub errors: Vec<Value>,
    pub positional_signals: Vec<Option<Entry>>,
    pub signals: Vec<Value>,
    pub keyed_signals: Vec<(u32, Option<Entry>)>,
    pub seeds: [u32; 2],
    pub keysets: Vec<Vec<String>>,
}

/// The four source report channels, without seeds, keysets or header context.
/// This is the decode-result shape a captured payload reduces to.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Channels<'a> {
    pub errors: &'a [Value],
    #[serde(rename = "positionalSignals")]
    pub positional_signals: &'a [Option<Entry>],
    pub signals: &'a [Value],
    #[serde(rename = "keyedSignals")]
    pub keyed_signals: &'a [(u32, Option<Entry>)],
}

/// An owned four-channel document: the shape `--channels` emits and encode
/// accepts. It carries no seeds, keysets or header context, so encoding takes
/// those from the caller or the loaded layout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChannelReport {
    pub errors: Vec<Value>,
    #[serde(rename = "positionalSignals")]
    pub positional_signals: Vec<Option<Entry>>,
    pub signals: Vec<Value>,
    #[serde(rename = "keyedSignals")]
    pub keyed_signals: Vec<(u32, Option<Entry>)>,
}

impl ChannelReport {
    /// Bounded typed parse of exactly the four channel keys.
    pub fn from_json(input: &[u8]) -> Result<Self, Error> {
        if input.len() > 128 * 1024 * 1024 {
            return Err(invalid("channel report JSON exceeds 128 MiB"));
        }
        super::typed_json::parse_typed_json(input).map_err(|e| Error::Invalid(e.to_string()))
    }

    /// Rebuild the full report for encoding.
    pub fn report(&self, seeds: [u32; 2], keysets: Vec<Vec<String>>) -> Report {
        Report {
            errors: self.errors.clone(),
            positional_signals: self.positional_signals.clone(),
            signals: self.signals.clone(),
            keyed_signals: self.keyed_signals.clone(),
            seeds,
            keysets,
        }
    }
}

impl Report {
    /// Owned four-channel document, directly encodable.
    pub fn channels_owned(&self) -> ChannelReport {
        ChannelReport {
            errors: self.errors.clone(),
            positional_signals: self.positional_signals.clone(),
            signals: self.signals.clone(),
            keyed_signals: self.keyed_signals.clone(),
        }
    }

    /// Project only the four report channels.
    pub fn channels(&self) -> Channels<'_> {
        Channels {
            errors: &self.errors,
            positional_signals: &self.positional_signals,
            signals: &self.signals,
            keyed_signals: &self.keyed_signals,
        }
    }

    /// Pack each nonempty value channel once, preserving raw extras and order.
    pub fn to_payload(&self) -> Result<Payload, Error> {
        self.validate()?;
        let defaults = SerializerOptions::default();
        let signals = SerializerOptions::outer_channel(self.keysets.clone());
        signals
            .validate()
            .map_err(|e| Error::Invalid(e.to_string()))?;
        let mut size = 0;
        let errors = pack_channel(&self.errors, &defaults, &mut size)?;
        let signals = pack_channel(&self.signals, &signals, &mut size)?;
        let mut container = Container {
            errors,
            signals,
            positional: vec![],
            keyed: vec![],
        };
        for entry in &self.positional_signals {
            container.positional.push(match entry {
                None => {
                    charge(&mut size, 2)?;
                    None
                }
                Some((v, extra)) => {
                    let encoded = pack_channel(v, &defaults, &mut size)?;
                    charge(&mut size, uleb(extra.len() as u64).len() + extra.len())?;
                    Some(PositionalEntry {
                        encoded,
                        extra: extra.clone(),
                    })
                }
            });
        }
        for (name, entry) in &self.keyed_signals {
            charge(&mut size, uleb(u64::from(*name)).len())?;
            let (encoded, extra) = match entry {
                None => {
                    charge(&mut size, 2)?;
                    (vec![], vec![])
                }
                Some((v, extra)) => {
                    let encoded = pack_channel(v, &defaults, &mut size)?;
                    charge(&mut size, uleb(extra.len() as u64).len() + extra.len())?;
                    (encoded, extra.clone())
                }
            };
            container.keyed.push(KeyedEntry {
                name: *name,
                encoded,
                extra,
            });
        }
        Ok(Payload {
            seeds: self.seeds,
            data: Data::Container { container },
            header_context: None,
        })
    }

    fn validate(&self) -> Result<(), Error> {
        if self
            .positional_signals
            .len()
            .saturating_add(self.keyed_signals.len())
            > MAX_CONTAINER_ENTRIES
        {
            return Err(invalid("report exceeds the 65536 container entry limit"));
        }
        let mut budget = ValueBudget::default();
        budget.values(&self.errors)?;
        budget.values(&self.signals)?;
        for entry in self
            .positional_signals
            .iter()
            .chain(self.keyed_signals.iter().map(|(_, e)| e))
            .flatten()
        {
            if entry.0.is_empty() && entry.1.is_empty() {
                return Err(invalid("empty report entry [[],[]] is ambiguous; use null for the missing-entry sentinel"));
            }
            budget.values(&entry.0)?;
            charge(&mut budget.storage, entry.1.len())?;
        }
        Ok(())
    }
}

fn pack_channel(
    values: &[Value],
    options: &SerializerOptions,
    size: &mut usize,
) -> Result<Vec<u8>, Error> {
    let bytes = if values.is_empty() {
        vec![]
    } else {
        pack(&Value::Array(values.to_vec()), options).map_err(|e| Error::Invalid(e.to_string()))?
    };
    charge(size, uleb(bytes.len() as u64).len() + bytes.len())?;
    Ok(bytes)
}

fn charge(size: &mut usize, n: usize) -> Result<(), Error> {
    *size = size
        .checked_add(n)
        .ok_or_else(|| invalid("report size overflow"))?;
    check_size(*size)
}

// A report-wide budget prevents many individually bounded channel decodes from
// accumulating unbounded expanded values. Walk iteratively, including content
// that semantic depth truncation will discard on encode.
#[derive(Default)]
struct ValueBudget {
    entries: usize,
    storage: usize,
}
impl ValueBudget {
    fn values(&mut self, values: &[Value]) -> Result<(), Error> {
        let mut pending = vec![];
        self.add_entries(values.len())?;
        pending.extend(values);
        while let Some(value) = pending.pop() {
            match value {
                Value::String(s) => charge(&mut self.storage, s.len())?,
                Value::Binary(bytes) => {
                    self.add_entries(bytes.len())?;
                    charge(&mut self.storage, bytes.len())?;
                }
                Value::Array(values) => {
                    self.add_entries(values.len())?;
                    pending.extend(values);
                }
                Value::Object(entries) => {
                    self.add_entries(entries.len())?;
                    for (key, value) in entries {
                        charge(&mut self.storage, key.len())?;
                        pending.push(value);
                    }
                }
                Value::Extension { payload, .. } => {
                    self.add_entries(1)?;
                    pending.push(payload);
                }
                _ => {}
            }
        }
        Ok(())
    }
    fn add_entries(&mut self, n: usize) -> Result<(), Error> {
        self.entries = self.entries.saturating_add(n);
        if self.entries > Limits::default().max_entries {
            return Err(invalid(
                "report generic values exceed the 65536 entry limit",
            ));
        }
        Ok(())
    }
}

impl PayloadCodec {
    /// Finalize a generic report; never formats headers or samples entropy.
    pub fn encode_report(&self, report: &Report) -> Result<String, Error> {
        let payload = report.to_payload()?;
        self.encode_bytes(&payload.plaintext()?, payload.seeds)
    }

    /// Counts and keysets are external facts, never inferred. Only canonical
    /// source report bytes are accepted; use `PayloadCodec::decode` for raw bytes.
    pub fn decode_report(
        &self,
        text: &str,
        positional_count: usize,
        keysets: Vec<Vec<String>>,
    ) -> Result<Report, Error> {
        let options = SerializerOptions::outer_channel(keysets.clone());
        options
            .validate()
            .map_err(|e| Error::Invalid(e.to_string()))?;
        let payload = self.decode(text)?.parse_container(positional_count)?;
        let Data::Container { container } = payload.data else {
            unreachable!()
        };
        let mut budget = ValueBudget::default();
        let mut channel = |bytes: &[u8],
                           options: &SerializerOptions,
                           label: &str|
         -> Result<Vec<Value>, Error> {
            if bytes.is_empty() {
                return Ok(vec![]);
            }
            let value = unpack_canonical(bytes, options).map_err(|e| Error::Invalid(format!(
                "{label}: {e}; supply the exact ordered keysets for signals; use PayloadCodec::decode for raw byte provenance"
            )))?;
            let Value::Array(values) = value else {
                return Err(Error::Invalid(format!("{label}: report value channels must be arrays; use PayloadCodec::decode for raw bytes")));
            };
            if values.is_empty() {
                return Err(Error::Invalid(format!("{label}: noncanonical report channel: empty arrays require a zero-length blob; use PayloadCodec::decode for raw bytes")));
            }
            budget.values(&values)?;
            Ok(values)
        };
        let defaults = SerializerOptions::default();
        let errors = channel(&container.errors, &defaults, "errors")?;
        let signals = channel(&container.signals, &options, "signals")?;
        let mut positional_signals = vec![];
        for (index, entry) in container.positional.into_iter().enumerate() {
            positional_signals.push(match entry {
                None => None,
                Some(entry) => Some((
                    channel(
                        &entry.encoded,
                        &defaults,
                        &format!("positionalSignals[{index}]"),
                    )?,
                    entry.extra,
                )),
            });
        }
        let mut keyed_signals = vec![];
        for (index, entry) in container.keyed.into_iter().enumerate() {
            let value = if entry.encoded.is_empty() && entry.extra.is_empty() {
                None
            } else {
                Some((
                    channel(&entry.encoded, &defaults, &format!("keyedSignals[{index}]"))?,
                    entry.extra,
                ))
            };
            keyed_signals.push((entry.name, value));
        }
        let report = Report {
            errors,
            positional_signals,
            signals,
            keyed_signals,
            seeds: payload.seeds,
            keysets,
        };
        report.validate()?;
        if self.encode_report(&report)? != text {
            return Err(invalid(
                "noncanonical report payload; use PayloadCodec::decode for raw byte provenance",
            ));
        }
        Ok(report)
    }
}
