use super::*;

/// Bounds used by both the raw and proof-backed evaluators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VmStringLimits {
    /// Maximum encoded input length in bytes. For accepted input the alphabet
    /// is ASCII, so this also bounds source characters.
    pub max_input_chars: usize,
    /// Maximum decoded bytes in either base64 operand.
    pub max_decoded_bytes: usize,
    /// Maximum UTF-16 code units in the result.
    pub max_output_units: usize,
    /// Maximum number of entries retained by a cache.
    pub max_cache_entries: usize,
    /// Maximum cache-key length in UTF-16 code units.
    pub max_cache_key_units: usize,
}

impl Default for VmStringLimits {
    fn default() -> Self {
        Self {
            max_input_chars: 64 * 1024,
            max_decoded_bytes: 64 * 1024,
            max_output_units: 64 * 1024,
            max_cache_entries: 4096,
            max_cache_key_units: 256,
        }
    }
}

/// A lossless sequence of JavaScript UTF-16 code units.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Utf16String(pub(super) Vec<u16>);

impl Utf16String {
    /// Wrap already-computed code units without validating surrogate pairing.
    pub fn from_units(units: Vec<u16>) -> Self {
        Self(units)
    }

    /// Borrow the units in evaluation order.
    pub fn units(&self) -> &[u16] {
        &self.0
    }

    /// Consume the value and return its units for a lossless literal encoder.
    pub fn into_units(self) -> Vec<u16> {
        self.0
    }

    /// Convert only when all code units form valid Unicode scalar sequences.
    /// This deliberately does not replace unpaired surrogates.
    pub fn to_string(&self) -> Result<String, Utf16Error> {
        String::from_utf16(&self.0).map_err(|_| Utf16Error)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("UTF-16 contains an unpaired surrogate")]
pub struct Utf16Error;

/// Explicit assumptions required before a JS-like cache can participate in a
/// fold.  A null-prototype, data-only cache has no getter, proxy, or inherited
/// property behavior to observe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheAssumption {
    /// The cache may have accessors, a prototype, or another observable shape.
    Unproven,
    /// `Object.create(null)` with only data reads/writes; see the evaluator
    /// contract for the exact source proof that establishes this.
    TrustedNullPrototypeDataOnly,
}

impl CacheAssumption {
    pub const fn is_trusted(self) -> bool {
        matches!(self, Self::TrustedNullPrototypeDataOnly)
    }
}

/// Explicit dependency facts for the four values passed to the helper.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DependencyAssumptions {
    pub string_table_data_only: bool,
    pub cache: CacheAssumption,
    pub decoder_is_proven_manual_base64: bool,
    pub from_char_code_is_intrinsic: bool,
}

impl DependencyAssumptions {
    pub const fn trusted_static() -> Self {
        Self {
            string_table_data_only: true,
            cache: CacheAssumption::TrustedNullPrototypeDataOnly,
            decoder_is_proven_manual_base64: true,
            from_char_code_is_intrinsic: true,
        }
    }

    pub(super) const fn is_trusted(self) -> bool {
        self.string_table_data_only
            && self.cache.is_trusted()
            && self.decoder_is_proven_manual_base64
            && self.from_char_code_is_intrinsic
    }
}

/// Rust-owned cache with an explicit JS-observability assumption.
#[derive(Debug, Clone)]
pub struct VmStringCache {
    pub(super) assumption: CacheAssumption,
    pub(super) entries: BTreeMap<String, VmStringCacheEntry>,
}

#[derive(Debug, Clone)]
pub(super) struct VmStringCacheEntry {
    pub(super) value: Utf16String,
    pub(super) decoded_bytes: usize,
}

impl VmStringCache {
    /// Construct a cache with an explicit observability assumption. An
    /// `Unproven` cache is retained for structural comparison but cannot be
    /// used by [`VmStringEvaluator::evaluate`].
    pub fn new(assumption: CacheAssumption) -> Self {
        Self {
            assumption,
            entries: BTreeMap::new(),
        }
    }

    /// Construct the only cache shape the evaluator accepts.
    pub fn trusted_null_prototype() -> Self {
        Self::new(CacheAssumption::TrustedNullPrototypeDataOnly)
    }

    pub fn assumption(&self) -> CacheAssumption {
        self.assumption
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Look up a value without recording the decoded-byte size retained in the
    /// internal entry. The evaluator uses the internal map directly so it can
    /// re-check those bounds on a hit.
    pub fn get(&self, key: &str) -> Option<&Utf16String> {
        self.entries.get(key).map(|entry| &entry.value)
    }
}

/// A data-only string table.  Unlike a JS array, it cannot invoke a getter or
/// consult a prototype while the evaluator reads an entry.
#[derive(Debug, Clone, Default)]
pub struct VmStringTable {
    pub(super) entries: Vec<String>,
}

impl VmStringTable {
    /// Create a table from already-decoded data. Invalid UTF-16 in a source
    /// table is rejected by the integration layer before this point.
    pub fn new(entries: Vec<String>) -> Self {
        Self { entries }
    }

    pub fn entries(&self) -> &[String] {
        &self.entries
    }
}

/// A request with explicit unknown states.  Unknown VM operands are a typed
/// non-fold, never an invented empty string or zero key.
#[derive(Debug, Clone, Copy)]
pub struct VmStringRequest<'a> {
    pub encoded: Option<&'a str>,
    pub key_index: Option<usize>,
    pub compute_only: bool,
}

/// Whether the result came from the trusted cache or was newly computed.
/// Whether the value came from the trusted cache or was computed now.
///
/// `cache_stored` is false for `computeOnly`; a cache-capacity failure returns
/// an error rather than a result with a silently dropped store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VmStringResult {
    CacheHit(Utf16String),
    Computed {
        value: Utf16String,
        cache_stored: bool,
    },
}

impl VmStringResult {
    pub fn value(&self) -> &Utf16String {
        match self {
            Self::CacheHit(value) | Self::Computed { value, .. } => value,
        }
    }
}
