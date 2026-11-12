use super::*;

/// Bounded evaluation failure. These are typed declines, not partial results.
///
/// Unknown operands, untrusted proofs/caches, invalid base64, and every limit
/// violation remain distinguishable so the integration report can explain why
/// a call stayed in the output.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum VmStringError {
    #[error("proof is required before folding vmDecodeString")]
    MissingProof,
    #[error("encoded input is unknown")]
    UnknownInput,
    #[error("key index is unknown")]
    UnknownKey,
    #[error("encoded input exceeds the {limit}-character bound")]
    InputTooLong { limit: usize },
    #[error("cache contains {actual} entries, exceeding the {limit}-entry bound")]
    CacheTooLarge { actual: usize, limit: usize },
    #[error("cache is not proven to be a safe null-prototype data cache")]
    UntrustedCache,
    #[error("key index {index} is outside the string table")]
    KeyOutOfBounds { index: usize },
    #[error("cache key exceeds the {limit}-unit bound")]
    CacheKeyTooLong { limit: usize },
    #[error("{which} is not admitted unpadded base64: {reason}")]
    InvalidBase64 {
        which: &'static str,
        reason: &'static str,
    },
    #[error("decoded operands must be non-empty and have equal lengths")]
    UnequalOrEmptyOperands,
    #[error("decoded operand exceeds the {limit}-byte bound")]
    DecodedInputTooLong { limit: usize },
    #[error("decoded output exceeds the {limit}-unit bound")]
    OutputTooLong { limit: usize },
    #[error("raw key bytes are unknown or empty")]
    RawKeyUnknown,
    #[error("raw encoded bytes are unknown or empty")]
    RawInputUnknown,
}

/// Bounded evaluator for the proof-backed helper.
#[derive(Debug, Clone, Copy)]
pub struct VmStringEvaluator {
    pub(super) limits: VmStringLimits,
}

impl VmStringEvaluator {
    pub const fn new(limits: VmStringLimits) -> Self {
        Self { limits }
    }

    pub const fn limits(self) -> VmStringLimits {
        self.limits
    }

    /// Evaluate one concrete helper call under the supplied proof and limits.
    ///
    /// The order mirrors the admitted helper: check the cache (unless
    /// `compute_only`), look up the key, decode both operands, XOR, then store
    /// unless `compute_only`. A cache hit is re-checked against the current
    /// decoded/output bounds because a cache may outlive a caller's limits.
    pub fn evaluate(
        &self,
        proof: Option<&VmStringProof>,
        request: VmStringRequest<'_>,
        table: &VmStringTable,
        cache: &mut VmStringCache,
    ) -> Result<VmStringResult, VmStringError> {
        let proof = proof.ok_or(VmStringError::MissingProof)?;
        if !proof.dependencies.is_trusted() || !cache.assumption.is_trusted() {
            return Err(VmStringError::UntrustedCache);
        }
        if cache.len() > self.limits.max_cache_entries {
            return Err(VmStringError::CacheTooLarge {
                actual: cache.len(),
                limit: self.limits.max_cache_entries,
            });
        }
        let encoded = request.encoded.ok_or(VmStringError::UnknownInput)?;
        let key_index = request.key_index.ok_or(VmStringError::UnknownKey)?;
        if encoded.len() > self.limits.max_input_chars {
            return Err(VmStringError::InputTooLong {
                limit: self.limits.max_input_chars,
            });
        }
        let cache_key = format!("{encoded},{key_index}");
        // Compute-only emission never looks up or stores this key. Its input
        // and output are still bounded independently of the cache limits.
        if !request.compute_only
            && cache_key.encode_utf16().count() > self.limits.max_cache_key_units
        {
            return Err(VmStringError::CacheKeyTooLong {
                limit: self.limits.max_cache_key_units,
            });
        }
        if !request.compute_only {
            if let Some(entry) = cache.entries.get(&cache_key) {
                if entry.decoded_bytes > self.limits.max_decoded_bytes {
                    return Err(VmStringError::DecodedInputTooLong {
                        limit: self.limits.max_decoded_bytes,
                    });
                }
                if entry.value.units().len() > self.limits.max_output_units {
                    return Err(VmStringError::OutputTooLong {
                        limit: self.limits.max_output_units,
                    });
                }
                return Ok(VmStringResult::CacheHit(entry.value.clone()));
            }
        }
        let key = table
            .entries
            .get(key_index)
            .ok_or(VmStringError::KeyOutOfBounds { index: key_index })?;
        let alphabet = proof.decoder_proof.alphabet;
        let key_bytes = decode_proven_base64(key, alphabet, &self.limits).map_err(|reason| {
            VmStringError::InvalidBase64 {
                which: "key",
                reason,
            }
        })?;
        let encoded_bytes =
            decode_proven_base64(encoded, alphabet, &self.limits).map_err(|reason| {
                VmStringError::InvalidBase64 {
                    which: "input",
                    reason,
                }
            })?;
        let value = evaluate_raw_xor(&encoded_bytes, &key_bytes, &self.limits)?;
        let cache_stored = if request.compute_only {
            false
        } else {
            if cache.len() >= self.limits.max_cache_entries {
                return Err(VmStringError::CacheTooLarge {
                    actual: cache.len() + 1,
                    limit: self.limits.max_cache_entries,
                });
            }
            cache.entries.insert(
                cache_key,
                VmStringCacheEntry {
                    decoded_bytes: key_bytes.len(),
                    value: value.clone(),
                },
            );
            true
        };
        Ok(VmStringResult::Computed {
            value,
            cache_stored,
        })
    }
}

/// Evaluate the XOR/UTF-16 arithmetic after both byte arrays have already
/// been proven or decoded.
///
/// The mask is `key[0] + encoded[0] & 255` with JavaScript's wrapping Number
/// addition semantics; output starts at index 1 and has `key.len() - 1` units.
/// Both operands must be non-empty and equal length. This utility intentionally
/// does not authorize a source helper, decoder, table, or cache by itself.
pub fn evaluate_raw_xor(
    encoded_bytes: &[u8],
    key_bytes: &[u8],
    limits: &VmStringLimits,
) -> Result<Utf16String, VmStringError> {
    if key_bytes.is_empty() {
        return Err(VmStringError::RawKeyUnknown);
    }
    if encoded_bytes.is_empty() {
        return Err(VmStringError::RawInputUnknown);
    }
    if key_bytes.len() != encoded_bytes.len() {
        return Err(VmStringError::UnequalOrEmptyOperands);
    }
    if key_bytes.len() > limits.max_decoded_bytes {
        return Err(VmStringError::DecodedInputTooLong {
            limit: limits.max_decoded_bytes,
        });
    }
    let output_len = key_bytes.len() - 1;
    if output_len > limits.max_output_units {
        return Err(VmStringError::OutputTooLong {
            limit: limits.max_output_units,
        });
    }
    // JS evaluates `keyBytes[0] + encodedBytes[0] & 255` as Number addition
    // followed by ToInt32 masking.  u8 wrapping_add is identical here.
    let xor_mask = key_bytes[0].wrapping_add(encoded_bytes[0]);
    let mut units = Vec::with_capacity(output_len);
    for (&encoded, &key) in encoded_bytes[1..].iter().zip(&key_bytes[1..]) {
        let code_unit = u16::from(encoded ^ key ^ xor_mask);
        units.push(code_unit);
    }
    Ok(Utf16String(units))
}

/// Decode only the standard, unpadded alphabet admitted by the proven manual
/// loop. This is deliberately not the crate's permissive URL-safe decoder.
///
/// Lengths with remainder 1 are rejected by the length check; remainders 2 and
/// 3 are decoded with the manual loop's bounded tail stores (missing characters
/// read as zero), so the final unused bits are not re-validated for
/// canonicality. A successful result is still bounded by `max_input_chars` and
/// `max_decoded_bytes`.
pub fn decode_manual_base64(input: &str, limits: &VmStringLimits) -> Result<Vec<u8>, &'static str> {
    decode_proven_base64(input, STANDARD_ALPHABET, limits)
}

/// [`decode_manual_base64`] in `alphabet`, the one a decoder proof names.
pub(super) fn decode_proven_base64(
    input: &str,
    alphabet: &str,
    limits: &VmStringLimits,
) -> Result<Vec<u8>, &'static str> {
    if input.is_empty() {
        return Err("empty input");
    }
    if input.len() > limits.max_input_chars {
        return Err("input exceeds bound");
    }
    if input.len() % 4 == 1 {
        return Err("length has impossible remainder");
    }
    let value = |byte: &u8| alphabet.bytes().position(|value| value == *byte);
    if input.as_bytes().iter().any(|byte| value(byte).is_none()) {
        return Err("character is outside the alphabet");
    }
    let output_len = input.len().checked_mul(3).ok_or("input size overflow")? / 4;
    if output_len > limits.max_decoded_bytes {
        return Err("decoded output exceeds bound");
    }
    let bytes = input.as_bytes();
    let mut output = Vec::with_capacity(output_len);
    let mut cursor = 0;
    while cursor < bytes.len() {
        let value = |byte| value(byte).map(|value| value as u8);
        let a = value(&bytes[cursor]).expect("validated alphabet");
        let b = bytes.get(cursor + 1).and_then(value).unwrap_or(0);
        let c = bytes.get(cursor + 2).and_then(value).unwrap_or(0);
        let d = bytes.get(cursor + 3).and_then(value).unwrap_or(0);
        output.push((a << 2) | (b >> 4));
        if cursor + 2 < bytes.len() {
            output.push((b << 4) | (c >> 2));
        }
        if cursor + 3 < bytes.len() {
            output.push((c << 6) | d);
        }
        cursor += 4;
    }
    Ok(output)
}

