//! Explicit snapshots for native dispatch and thread commands.
use crate::{SymbolicValue, ThreadDescriptor, ThreadInputs, TraceLimits};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, io::Read, path::Path};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TraceValue {
    Undefined,
    Null,
    Boolean {
        value: bool,
    },
    Number {
        #[serde(default, deserialize_with = "deserialize_number")]
        value: Option<f64>,
        #[serde(default)]
        encoding: Option<NumberEncoding>,
    },
    String {
        value: String,
    },
    Object {
        identity: u64,
    },
    External {
        name: String,
    },
    /// Output-only representation. Requests containing this value are
    /// rejected because a node belongs to another trace arena.
    Computed {
        node: u32,
    },
    /// Output-only representation. Requests containing this value are
    /// rejected rather than treating it as a concrete value.
    Unknown {
        reason: String,
    },
}

// Internally tagged enums buffer fields through Serde's content model. Decode
// via serde_json::Number so arbitrary_precision decimal tokens keep working.
fn deserialize_number<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<f64>, D::Error> {
    let number = Option::<serde_json::Number>::deserialize(deserializer)?;
    number
        .map(|number| {
            number
                .as_f64()
                .ok_or_else(|| serde::de::Error::custom("number is outside f64 range"))
        })
        .transpose()
}

/// Explicit encodings for IEEE-754 values that JSON cannot carry faithfully.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NumberEncoding {
    NegativeZero,
    #[serde(rename = "nan")]
    NaN,
    PositiveInfinity,
    NegativeInfinity,
}

impl TraceValue {
    /// Decode a caller-supplied value without coercion.
    pub fn to_symbolic(&self) -> Result<SymbolicValue, String> {
        self.to_symbolic_input()
    }

    pub fn validate_number(
        value: Option<f64>,
        encoding: Option<NumberEncoding>,
    ) -> Result<f64, String> {
        match (value, encoding) {
            (Some(value), None)
                if value.is_finite() && !(value == 0.0 && value.is_sign_negative()) =>
            {
                Ok(value)
            }
            (None, Some(NumberEncoding::NegativeZero)) => Ok(-0.0),
            (None, Some(NumberEncoding::NaN)) => Ok(f64::NAN),
            (None, Some(NumberEncoding::PositiveInfinity)) => Ok(f64::INFINITY),
            (None, Some(NumberEncoding::NegativeInfinity)) => Ok(f64::NEG_INFINITY),
            (Some(_), None) => Err(
                "number value must be finite and negative zero must use encoding=negative-zero"
                    .into(),
            ),
            (Some(_), Some(_)) => Err("number must use value or encoding, not both".into()),
            (None, None) => Err("number requires value or explicit encoding".into()),
        }
    }

    pub fn to_symbolic_input(&self) -> Result<SymbolicValue, String> {
        match self {
            Self::Undefined => Ok(SymbolicValue::Undefined),
            Self::Null => Ok(SymbolicValue::Null),
            Self::Boolean { value } => Ok(SymbolicValue::Boolean(*value)),
            Self::Number { value, encoding } => Ok(SymbolicValue::Number(Self::validate_number(
                *value, *encoding,
            )?)),
            Self::String { value } => Ok(SymbolicValue::String(value.clone())),
            Self::Object { identity } => Ok(SymbolicValue::Object {
                identity: *identity,
            }),
            Self::External { name } => Ok(SymbolicValue::External { name: name.clone() }),
            Self::Computed { .. } => Err("computed values are not valid caller inputs".into()),
            Self::Unknown { .. } => Err("unknown values are not valid caller inputs".into()),
        }
    }
}

impl TryFrom<&TraceValue> for SymbolicValue {
    type Error = String;

    fn try_from(value: &TraceValue) -> Result<Self, Self::Error> {
        value.to_symbolic_input()
    }
}

impl From<&SymbolicValue> for TraceValue {
    fn from(value: &SymbolicValue) -> Self {
        match value {
            SymbolicValue::Undefined => Self::Undefined,
            SymbolicValue::Null => Self::Null,
            SymbolicValue::Boolean(value) => Self::Boolean { value: *value },
            SymbolicValue::Number(value) if value.is_nan() => Self::Number {
                value: None,
                encoding: Some(NumberEncoding::NaN),
            },
            SymbolicValue::Number(value) if value.is_infinite() && value.is_sign_positive() => {
                Self::Number {
                    value: None,
                    encoding: Some(NumberEncoding::PositiveInfinity),
                }
            }
            SymbolicValue::Number(value) if value.is_infinite() => Self::Number {
                value: None,
                encoding: Some(NumberEncoding::NegativeInfinity),
            },
            SymbolicValue::Number(value) if *value == 0.0 && value.is_sign_negative() => {
                Self::Number {
                    value: None,
                    encoding: Some(NumberEncoding::NegativeZero),
                }
            }
            SymbolicValue::Number(value) => Self::Number {
                value: Some(*value),
                encoding: None,
            },
            SymbolicValue::String(value) => Self::String {
                value: value.clone(),
            },
            SymbolicValue::Object { identity } => Self::Object {
                identity: *identity,
            },
            SymbolicValue::External { name } => Self::External { name: name.clone() },
            SymbolicValue::Computed { node } => Self::Computed { node: *node },
            SymbolicValue::Unknown { reason } => Self::Unknown {
                reason: reason.clone(),
            },
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Snapshot {
    pub descriptor: Option<Descriptor>,
    captured_memory: BTreeMap<u32, TraceValue>,
    incoming_stack: Vec<Option<TraceValue>>,
    arguments_object: Option<TraceValue>,
    self_value: Option<TraceValue>,
    receiver: Option<TraceValue>,
    arguments: Vec<TraceValue>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Descriptor {
    initial_slots: Vec<u32>,
    captured_slots: Vec<u32>,
    argument_slots: Vec<u32>,
    arguments_slot: Option<u32>,
    result_slot: Option<u32>,
}
impl Descriptor {
    pub fn into_descriptor(self) -> ThreadDescriptor {
        ThreadDescriptor {
            initial_slots: self.initial_slots,
            captured_slots: self.captured_slots,
            argument_slots: self.argument_slots,
            arguments_slot: self.arguments_slot,
            result_slot: self.result_slot,
            ..ThreadDescriptor::default()
        }
    }
}
impl Snapshot {
    pub fn read(path: &Path) -> Result<Self, String> {
        let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut bytes = Vec::new();
        file.take(16 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > 16 * 1024 * 1024 {
            return Err("input snapshot exceeds 16 MiB".into());
        }
        serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))
    }
    pub fn into_inputs(
        self,
        limits: TraceLimits,
    ) -> Result<(ThreadInputs, Vec<SymbolicValue>), String> {
        let convert =
            |value: Option<TraceValue>| value.as_ref().map(TraceValue::to_symbolic).transpose();
        let inputs = ThreadInputs {
            captured_memory: self
                .captured_memory
                .iter()
                .map(|(slot, value)| Ok((*slot, value.to_symbolic()?)))
                .collect::<Result<_, String>>()?,
            incoming_stack: self
                .incoming_stack
                .into_iter()
                .map(convert)
                .collect::<Result<_, _>>()?,
            arguments_object: convert(self.arguments_object)?,
            self_value: convert(self.self_value)?,
            receiver: convert(self.receiver)?,
            limits,
            max_invocation_arguments: None,
        };
        let arguments = self
            .arguments
            .iter()
            .map(TraceValue::to_symbolic)
            .collect::<Result<_, _>>()?;
        Ok((inputs, arguments))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn special_numbers_round_trip_without_json_null_coercion() {
        for number in [-0.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 100.0] {
            let tagged = TraceValue::from(&SymbolicValue::Number(number));
            let json = serde_json::to_string(&tagged).unwrap();
            let value: TraceValue = serde_json::from_str(&json).unwrap();
            let SymbolicValue::Number(result) = value.to_symbolic().unwrap() else {
                panic!("number expected");
            };
            assert!(number.is_nan() && result.is_nan() || number.to_bits() == result.to_bits());
        }
    }
    #[test]
    fn snapshot_distinguishes_holes_and_rejects_unusable_inputs() {
        let snapshot: Snapshot =
            serde_json::from_str(r#"{"incoming_stack":[null,{"kind":"undefined"}]}"#).unwrap();
        let (inputs, _) = snapshot.into_inputs(TraceLimits::default()).unwrap();
        assert_eq!(
            inputs.incoming_stack,
            [None, Some(SymbolicValue::Undefined)]
        );
        for input in [
            r#"{"arguments":[{"kind":"computed","node":0}]}"#,
            r#"{"arguments":[{"kind":"unknown","reason":"guess"}]}"#,
            r#"{"arguments":[{"kind":"number","value":1,"encoding":"nan"}]}"#,
        ] {
            let snapshot: Snapshot = serde_json::from_str(input).unwrap();
            assert!(snapshot.into_inputs(TraceLimits::default()).is_err());
        }
        assert!(serde_json::from_str::<Snapshot>(r#"{"argumnts":[]}"#).is_err());
    }
}
