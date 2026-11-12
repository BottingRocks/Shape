//! Data-driven signal layouts.
//!
//! The write program of a real signal lives in its collector. Until those
//! mappings are recovered, this module lets the codec work on arbitrary sample
//! data: a [`Layout`] is a list of [`Field`]s, and the value for one signal is
//! a JSON object keyed by field name.
//!
//! ```json
//! {
//!   "fields": [
//!     {"kind": "array", "name": "responses", "reverse": true,
//!      "element": {"kind": "number", "name": "item"}},
//!     {"kind": "number", "name": "status"},
//!     {"kind": "optional", "name": "note", "tag": 130,
//!      "inner": {"kind": "string", "name": "noteValue"}},
//!     {"kind": "packed", "name": "flags", "bits": ["webdriver", "phantom"]}
//!   ]
//! }
//! ```
//!
//! The fields map one-to-one onto the writers:
//!
//! | field | writer | value |
//! |---|---|---|
//! | `number` | number writer | JSON number |
//! | `string` | string writer | JSON string |
//! | `byte` | one masked byte | JSON number `0..=255` |
//! | `packed` | one masked byte | array of booleans, shift = position |
//! | `optional` | tag byte or inner value | `null`, or the inner value |
//! | `array` | number writer + elements | array; `reverse` writes it back to front |
//!
//! When a real signal is mapped, its collector becomes a layout of this shape,
//! so the encoder never has to change.

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize};

use super::{decode_signal, try_encode_signal, CodecError, SignalReader, SignalWriter};

/// A decoded or sample signal value.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum SignalValue {
    /// The detector's `null`/`undefined` branch.
    Null,
    /// A boolean (packed bits, optional flags).
    Bool(bool),
    /// A number (int32 or float, as the number writer decides).
    Number(f64),
    /// A string.
    Text(String),
    /// An array.
    Array(Vec<SignalValue>),
    /// A named record. Layouts use this at the top level.
    Object(BTreeMap<String, SignalValue>),
}

impl<'de> Deserialize<'de> for SignalValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Deserialize through `serde_json::Value`: this crate enables
        // `arbitrary_precision`, and serde's untagged buffering can then read a
        // JSON number as an object. `Value` keeps numbers as numbers.
        let value = serde_json::Value::deserialize(deserializer)?;
        Ok(Self::from_json(value))
    }
}

impl SignalValue {
    fn from_json(value: serde_json::Value) -> Self {
        match value {
            serde_json::Value::Null => Self::Null,
            serde_json::Value::Bool(flag) => Self::Bool(flag),
            serde_json::Value::Number(number) => Self::Number(number.as_f64().unwrap_or(f64::NAN)),
            serde_json::Value::String(text) => Self::Text(text),
            serde_json::Value::Array(items) => {
                Self::Array(items.into_iter().map(Self::from_json).collect())
            }
            serde_json::Value::Object(fields) => Self::Object(
                fields
                    .into_iter()
                    .map(|(name, value)| (name, Self::from_json(value)))
                    .collect(),
            ),
        }
    }

    /// Build a record from `(name, value)` pairs.
    pub fn object<const N: usize>(entries: [(&str, SignalValue); N]) -> Self {
        Self::Object(
            entries
                .into_iter()
                .map(|(name, value)| (name.to_owned(), value))
                .collect(),
        )
    }

    /// The value stored under `name`, if this is a record.
    pub fn get(&self, name: &str) -> Option<&Self> {
        match self {
            Self::Object(map) => map.get(name),
            _ => None,
        }
    }

    /// `Some` when this is [`SignalValue::Number`].
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Self::Number(value) => Some(*value),
            _ => None,
        }
    }

    /// `Some` when this is [`SignalValue::Text`].
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Text(value) => Some(value),
            _ => None,
        }
    }

    /// `Some` when this is [`SignalValue::Array`].
    pub fn as_array(&self) -> Option<&[Self]> {
        match self {
            Self::Array(items) => Some(items),
            _ => None,
        }
    }
}

impl From<f64> for SignalValue {
    fn from(value: f64) -> Self {
        Self::Number(value)
    }
}

impl From<i32> for SignalValue {
    fn from(value: i32) -> Self {
        Self::Number(f64::from(value))
    }
}

impl From<bool> for SignalValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<&str> for SignalValue {
    fn from(value: &str) -> Self {
        Self::Text(value.to_owned())
    }
}

impl From<String> for SignalValue {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<Vec<SignalValue>> for SignalValue {
    fn from(value: Vec<SignalValue>) -> Self {
        Self::Array(value)
    }
}

/// One bit position in a [`Field::Packed`] byte.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BitSpec {
    /// The bit at its index in `bits`.
    Named(String),
    /// The bit at an explicit shift.
    Shifted {
        /// Display name; the decoded value is positional.
        name: String,
        /// Bit position `0..=7`.
        shift: u8,
    },
}

impl BitSpec {
    /// The bit's name.
    pub fn name(&self) -> &str {
        match self {
            Self::Named(name) | Self::Shifted { name, .. } => name,
        }
    }

    /// The bit's position, defaulting to its index.
    pub fn shift(&self, index: usize) -> u8 {
        match self {
            Self::Named(_) => index as u8,
            Self::Shifted { shift, .. } => *shift,
        }
    }
}

/// How to write (or read) one field of a signal body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Field {
    /// Number writer.
    Number {
        /// Field name in the value object.
        name: String,
    },
    /// String writer.
    #[serde(rename = "string")]
    Text {
        /// Field name in the value object.
        name: String,
    },
    /// One masked byte, `0..=255`.
    Byte {
        /// Field name in the value object.
        name: String,
    },
    /// One masked byte with bitfields.
    Packed {
        /// Field name in the value object.
        name: String,
        /// Bit positions, in value order.
        bits: Vec<BitSpec>,
    },
    /// A `null` tag byte, or the inner field when the value is not null.
    Optional {
        /// Field name in the value object.
        name: String,
        /// The masked tag byte for `null`/`undefined`.
        tag: u8,
        /// Encoding of the non-null value.
        inner: Box<Field>,
    },
    /// Number-writer count, then `count` elements (optionally back to front).
    Array {
        /// Field name in the value object.
        name: String,
        /// Write elements highest index first (the XHR collector does).
        #[serde(default)]
        reverse: bool,
        /// Encoding of one element.
        element: Box<Field>,
    },
}

impl Field {
    /// The name this field reads from / writes to.
    pub fn name(&self) -> &str {
        match self {
            Self::Number { name }
            | Self::Text { name }
            | Self::Byte { name }
            | Self::Packed { name, .. }
            | Self::Optional { name, .. }
            | Self::Array { name, .. } => name,
        }
    }
}

/// A signal body's field order.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    /// Fields in write order.
    #[serde(default)]
    pub fields: Vec<Field>,
}

/// Encode sample values with a layout.
pub fn encode_layout(
    layout: &Layout,
    values: &SignalValue,
    token_hash: i32,
    seed: u32,
    xor: u32,
) -> Result<Vec<u8>, CodecError> {
    let SignalValue::Object(record) = values else {
        return Err(CodecError::WrongValue {
            field: "<root>".to_owned(),
            expected: "object",
        });
    };
    try_encode_signal(token_hash, seed, xor, |writer| {
        for field in &layout.fields {
            let value = record
                .get(field.name())
                .ok_or_else(|| CodecError::MissingField(field.name().to_owned()))?;
            write_field(writer, field, value)?;
        }
        Ok(())
    })
}

/// Decode a body with a layout, requiring exact consumption.
pub fn decode_layout(
    layout: &Layout,
    extra: &[u8],
    token_hash: i32,
    xor: u32,
) -> Result<(SignalValue, u32), CodecError> {
    let (record, seed) = decode_signal(extra, token_hash, xor, |reader| {
        let mut record = BTreeMap::new();
        for field in &layout.fields {
            record.insert(field.name().to_owned(), read_field(reader, field)?);
        }
        Ok(record)
    })?;
    Ok((SignalValue::Object(record), seed))
}

fn write_field(
    writer: &mut SignalWriter,
    field: &Field,
    value: &SignalValue,
) -> Result<(), CodecError> {
    match field {
        Field::Number { name } => writer.number(expect_number(name, value)?),
        Field::Text { name } => writer.string(expect_text(name, value)?),
        Field::Byte { name } => {
            let number = expect_number(name, value)?;
            if !(0.0..=255.0).contains(&number) || number.fract() != 0.0 {
                return Err(CodecError::ByteOutOfRange {
                    field: name.clone(),
                    value: number,
                });
            }
            writer.byte(number as u8);
        }
        Field::Packed { name, bits } => {
            let items = expect_array(name, value)?;
            if items.len() != bits.len() {
                return Err(CodecError::WrongValue {
                    field: name.clone(),
                    expected: "array of one boolean per bit",
                });
            }
            let mut byte = 0u8;
            for (index, item) in items.iter().enumerate() {
                if expect_bool(name, item)? {
                    byte |= 1 << bits[index].shift(index);
                }
            }
            writer.byte(byte);
        }
        Field::Optional { tag, inner, .. } => match value {
            SignalValue::Null => writer.byte(*tag),
            other => write_field(writer, inner, other)?,
        },
        Field::Array {
            name,
            reverse,
            element,
        } => {
            let items = expect_array(name, value)?;
            writer.number(items.len() as f64);
            if *reverse {
                for item in items.iter().rev() {
                    write_field(writer, element, item)?;
                }
            } else {
                for item in items {
                    write_field(writer, element, item)?;
                }
            }
        }
    }
    Ok(())
}

fn read_field(reader: &mut SignalReader<'_>, field: &Field) -> Result<SignalValue, CodecError> {
    Ok(match field {
        Field::Number { .. } => SignalValue::Number(reader.number()?),
        Field::Text { .. } => SignalValue::Text(reader.string()?),
        Field::Byte { .. } => SignalValue::Number(f64::from(reader.byte()?)),
        Field::Packed { bits, .. } => {
            let byte = reader.byte()?;
            SignalValue::Array(
                (0..bits.len())
                    .map(|index| SignalValue::Bool((byte >> bits[index].shift(index)) & 1 == 1))
                    .collect(),
            )
        }
        Field::Optional { tag, inner, .. } => {
            if reader.peek_tag(*tag)? {
                SignalValue::Null
            } else {
                read_field(reader, inner)?
            }
        }
        Field::Array {
            name,
            reverse,
            element,
        } => {
            let count = reader.number()?;
            // Each element consumes at least one byte, so the remaining body
            // bounds the count and a corrupt length cannot allocate freely.
            if count < 0.0 || count.fract() != 0.0 || count > reader.remaining() as f64 {
                return Err(CodecError::ArrayCount {
                    field: name.clone(),
                    count,
                });
            }
            let count = count as usize;
            let mut items = Vec::with_capacity(count);
            for _ in 0..count {
                items.push(read_field(reader, element)?);
            }
            if *reverse {
                items.reverse();
            }
            SignalValue::Array(items)
        }
    })
}

fn expect_number(field: &str, value: &SignalValue) -> Result<f64, CodecError> {
    value.as_number().ok_or_else(|| CodecError::WrongValue {
        field: field.to_owned(),
        expected: "number",
    })
}

fn expect_text<'a>(field: &str, value: &'a SignalValue) -> Result<&'a str, CodecError> {
    value.as_str().ok_or_else(|| CodecError::WrongValue {
        field: field.to_owned(),
        expected: "string",
    })
}

fn expect_array<'a>(field: &str, value: &'a SignalValue) -> Result<&'a [SignalValue], CodecError> {
    value.as_array().ok_or_else(|| CodecError::WrongValue {
        field: field.to_owned(),
        expected: "array",
    })
}

fn expect_bool(field: &str, value: &SignalValue) -> Result<bool, CodecError> {
    match value {
        SignalValue::Bool(flag) => Ok(*flag),
        SignalValue::Number(number) if *number == 0.0 => Ok(false),
        SignalValue::Number(number) if *number == 1.0 => Ok(true),
        _ => Err(CodecError::WrongValue {
            field: field.to_owned(),
            expected: "boolean",
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(text: &str) -> Vec<u8> {
        (0..text.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&text[i..i + 2], 16).expect("hex"))
            .collect()
    }

    fn xhr_layout() -> Layout {
        serde_json::from_str(
            r#"{
                "fields": [
                    {"kind": "array", "name": "responses", "reverse": true,
                     "element": {"kind": "number", "name": "item"}},
                    {"kind": "number", "name": "cpHash"},
                    {"kind": "number", "name": "xhrStatus"},
                    {"kind": "number", "name": "xhrLoadEndTs"},
                    {"kind": "number", "name": "xhrSendTs"},
                    {"kind": "byte", "name": "hasNetworkError"},
                    {"kind": "string", "name": "xhrResponse"}
                ]
            }"#,
        )
        .expect("layout json")
    }

    fn xhr_values() -> SignalValue {
        serde_json::from_str(
            r#"{
                "responses": [418, 3.5, -7],
                "cpHash": 123456789,
                "xhrStatus": 200,
                "xhrLoadEndTs": 1699999999999,
                "xhrSendTs": 1699999999000,
                "hasNetworkError": 0,
                "xhrResponse": "{\"ok\":true}"
            }"#,
        )
        .expect("value json")
    }

    #[test]
    fn layout_reproduces_the_hand_written_program() {
        let extra = encode_layout(
            &xhr_layout(),
            &xhr_values(),
            403_968_531,
            0x1122_3344,
            2_363_885_845,
        )
        .expect("encode");
        assert_eq!(
            extra,
            hex("11223344c70b696edd5caca9105ef1a9b5cdb26b65f5401bbaf771606ada7407a3fbcfbc9755b8e82344581eb676c5e28ceffc9f71825d")
        );
        let (decoded, seed) =
            decode_layout(&xhr_layout(), &extra, 403_968_531, 2_363_885_845).expect("decode");
        assert_eq!(seed, 0x1122_3344);
        assert_eq!(decoded, xhr_values());
    }

    #[test]
    fn packed_and_optional_round_trip() {
        let layout: Layout = serde_json::from_str(
            r#"{
                "fields": [
                    {"kind": "packed", "name": "flags",
                     "bits": ["webdriver", {"name": "phantom", "shift": 3}]},
                    {"kind": "optional", "name": "note", "tag": 130,
                     "inner": {"kind": "string", "name": "noteValue"}},
                    {"kind": "optional", "name": "count", "tag": 131,
                     "inner": {"kind": "number", "name": "countValue"}}
                ]
            }"#,
        )
        .expect("layout");

        for note in [SignalValue::Null, SignalValue::from("present")] {
            for count in [SignalValue::Null, SignalValue::from(7)] {
                let values = SignalValue::object([
                    (
                        "flags",
                        SignalValue::Array(vec![SignalValue::Bool(true), SignalValue::Bool(false)]),
                    ),
                    ("note", note.clone()),
                    ("count", count.clone()),
                ]);
                let extra = encode_layout(&layout, &values, 1, 0x0a0b_0c0d, 99).expect("encode");
                let (decoded, _) = decode_layout(&layout, &extra, 1, 99).expect("decode");
                assert_eq!(decoded, values, "note={note:?} count={count:?}");
            }
        }
    }

    #[test]
    fn arrays_of_strings_keep_their_order() {
        let layout: Layout = serde_json::from_str(
            r#"{"fields": [
                {"kind": "array", "name": "values",
                 "element": {"kind": "string", "name": "item"}}
            ]}"#,
        )
        .expect("layout");
        let values = SignalValue::object([(
            "values",
            SignalValue::Array(vec![
                SignalValue::from("first"),
                SignalValue::from("second"),
                SignalValue::from("first"),
            ]),
        )]);
        let extra = encode_layout(&layout, &values, 2, 3, 4).expect("encode");
        let (decoded, _) = decode_layout(&layout, &extra, 2, 4).expect("decode");
        assert_eq!(decoded, values);
    }

    #[test]
    fn layout_reports_missing_and_mistyped_values() {
        let layout = xhr_layout();
        let mut values = xhr_values();
        let SignalValue::Object(record) = &mut values else {
            panic!("object");
        };
        record.remove("xhrStatus");
        assert!(matches!(
            encode_layout(&layout, &values, 0, 0, 0),
            Err(CodecError::MissingField(name)) if name == "xhrStatus"
        ));

        let wrong = SignalValue::object([("responses", SignalValue::from(1))]);
        assert!(matches!(
            encode_layout(&layout, &wrong, 0, 0, 0),
            Err(CodecError::WrongValue { .. })
        ));

        let byte_layout: Layout =
            serde_json::from_str(r#"{"fields": [{"kind": "byte", "name": "status"}]}"#)
                .expect("layout");
        let too_big = SignalValue::object([("status", SignalValue::from(300))]);
        assert!(matches!(
            encode_layout(&byte_layout, &too_big, 0, 0, 0),
            Err(CodecError::ByteOutOfRange { .. })
        ));

        let root = SignalValue::from(1);
        assert!(matches!(
            encode_layout(&layout, &root, 0, 0, 0),
            Err(CodecError::WrongValue { field, .. }) if field == "<root>"
        ));
    }

    #[test]
    fn shipped_sample_file_round_trips() {
        // Synthetic codec regression data retained from the former JSON driver.
        #[derive(Deserialize)]
        struct SampleSignal {
            name: String,
            seed: u32,
            xor: u32,
            layout: Layout,
            values: SignalValue,
        }
        #[derive(Deserialize)]
        struct SampleFile {
            signals: Vec<SampleSignal>,
        }
        let sample: SampleFile =
            serde_json::from_str(include_str!("../../../tests/fixtures/sdk/generator/signal-codec.json"))
                .expect("sample json");
        assert_eq!(sample.signals.len(), 3);
        for signal in &sample.signals {
            let extra = encode_layout(
                &signal.layout,
                &signal.values,
                403_968_531,
                signal.seed,
                signal.xor,
            )
            .expect("encode");
            let (value, seed) =
                decode_layout(&signal.layout, &extra, 403_968_531, signal.xor).expect("decode");
            assert_eq!(seed, signal.seed, "{}", signal.name);
            assert_eq!(value, signal.values, "{}", signal.name);
        }
    }

    #[test]
    fn layout_rejects_a_bogus_array_count() {
        let layout: Layout = serde_json::from_str(
            r#"{"fields": [
                {"kind": "array", "name": "items",
                 "element": {"kind": "number", "name": "item"}}
            ]}"#,
        )
        .expect("layout");
        // A body whose count (1000) cannot fit the empty remainder.
        let extra = super::super::encode_signal(0, 7, 0, |writer| writer.number(1000.0));
        assert!(matches!(
            decode_layout(&layout, &extra, 0, 0),
            Err(CodecError::ArrayCount { count, .. }) if count == 1000.0
        ));
    }
}
