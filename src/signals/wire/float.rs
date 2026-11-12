//! Conditional native IEEE proof and externally selected wire byte order.
use super::{algorithms, AlgorithmProof, WireError};
use crate::signals::codec::{SignalReader, SignalWriter};
use serde::{Deserialize, Serialize};
#[path = "float_proof.rs"]
mod dependencies;
pub use dependencies::ProofSite;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FloatByteOrder {
    #[default]
    Unknown,
    Little,
    Big,
}

/// External runtime configuration, never inferred from values or source text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TypedDecoderOptions {
    #[serde(default)]
    pub float_byte_order: FloatByteOrder,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NumberFloatMetadata {
    pub marker: u8,
    pub width: u8,
    pub representation: String,
    pub native_byte_order: String,
    pub fallback_byte_order: String,
    pub runtime_prerequisites: Vec<String>,
    pub minimum_input: String,
    pub writer: AlgorithmProof,
    pub native_dependencies: AlgorithmProof,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provenance: Vec<ProofSite>,
}

pub(super) fn certify(source: &str, writer: &AlgorithmProof) -> NumberFloatMetadata {
    let candidate = writer
        .span
        .and_then(|s| source.get(s.start as usize..s.end as usize).map(|t| (s, t)));
    let mut writer = algorithms::proof(
        candidate,
        include_str!("reference/number.js"),
        "ieee64-whole-number-writer/1",
        false,
    );
    let mut dependencies = algorithms::proof(None, "", "native-f64-backing-bytes/2", false);
    let mut provenance = Vec::new();
    match writer
        .span
        .ok_or("selected writer span is absent")
        .and_then(|span| dependencies::certify(source, span, &mut writer))
    {
        Ok((native, sites)) => {
            dependencies = native;
            provenance = sites;
        }
        Err(reason) => {
            dependencies.reason = Some(reason.into());
        }
    }
    NumberFloatMetadata {
        marker: 128,
        width: 8,
        representation: "IEEE-754 binary64 backing bytes".into(),
        native_byte_order: "host-native; external configuration required".into(),
        fallback_byte_order: "reviewed reference little; selected source software fallback is not certified".into(),
        runtime_prerequisites: vec![
            "unmodified globalThis.Float64Array and globalThis.Uint8Array intrinsics".into(),
            "native initialization completed before use; active unmodified eight-byte scratch and native branch execution".into(),
            "no concurrent or reentrant mutation of scratch/backing views during byte emission".into(),
            "configured byte order equals collecting runtime native byte order".into(),
            "NaN wire bits describe stored backing bytes; payload behavior of a new JavaScript NaN store is runtime dependent".into(),
        ],
        minimum_input: "whole number helper plus scratch/byte-array constructor and alias provenance; confirmed collecting runtime native path and byte order".into(),
        writer,
        native_dependencies: dependencies,
        provenance,
    }
}

impl NumberFloatMetadata {
    pub(super) fn format(&self, options: TypedDecoderOptions) -> Result<FloatByteOrder, WireError> {
        if self.writer.codec.is_none() || self.native_dependencies.codec.is_none() {
            return Err(WireError::Unsupported("source float primitive/dependencies are not certified; whole native scratch/alias proof required (software fallback unsupported)".into()));
        }
        match options.float_byte_order {
            FloatByteOrder::Unknown => Err(WireError::Unsupported("float runtime byte order is unknown; explicitly configure Little or Big for the proven native path".into())),
            order => Ok(order),
        }
    }
}

pub(super) fn integer_domain(value: f64) -> bool {
    value.is_finite() && value as i32 as f64 == value
}

pub(super) fn read(
    reader: &mut SignalReader<'_>,
    format: &Result<FloatByteOrder, WireError>,
) -> Result<f64, WireError> {
    // Peek on a clone: a refusal must not consume marker or advance the mask.
    if reader.clone().peek_tag(128).map_err(WireError::codec)? {
        let order = format.as_ref().map_err(Clone::clone)?;
        let number = reader
            .number_with_float_order(*order == FloatByteOrder::Big)
            .map_err(WireError::codec)?;
        if integer_domain(number) {
            return Err(WireError::Codec);
        }
        Ok(number)
    } else {
        reader.number().map_err(WireError::codec)
    }
}

pub(super) fn write(
    writer: &mut SignalWriter,
    bits: u64,
    format: &Result<FloatByteOrder, WireError>,
) -> Result<(), WireError> {
    let value = f64::from_bits(bits);
    if integer_domain(value) {
        writer.number(value);
    } else {
        let order = format.as_ref().map_err(Clone::clone)?;
        writer.number_with_float_order(value, *order == FloatByteOrder::Big);
    }
    Ok(())
}

#[cfg(test)]
#[path = "float_tests.rs"]
mod tests;
