//! Shared bounded WebAssembly decoding, parsing, and validation.
//!
//! The callers deliberately project this summary differently.  In particular,
//! runtime-config and layout require validation while actual-properties keeps
//! its historical parse-only acceptance.  Keeping both outcomes here prevents
//! those policy differences from turning into four independent decoders.
//!
//! The neutral ABI projection (imports, exports, and the two memory booleans)
//! is intentionally separate from the optional detailed boundary projection.
//! The old simple callers only read the import, memory, and export sections;
//! they never parsed type, table, element, or body details.  A detailed parse
//! error must therefore never erase or reject their ABI result.

use wasmparser::{ExternalKind, Imports, Parser, Payload, TypeRef, Validator};

const STANDARD_ALPHABET: &[u8] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Base64Policy {
    UrlSafe,
    Standard { limit: usize },
}

/// Decode one of the two historical kernel base64 policies.
pub(crate) fn decode_base64(input: &str, policy: Base64Policy) -> Result<Vec<u8>, String> {
    match policy {
        Base64Policy::UrlSafe => decode_url_safe(input),
        Base64Policy::Standard { limit } => decode_standard(input, limit),
    }
}

fn decode_url_safe(input: &str) -> Result<Vec<u8>, String> {
    if input.is_empty() {
        return Err("empty base64 input".into());
    }
    let bytes = input.as_bytes();
    if bytes.len() % 4 == 1 {
        return Err("base64 length has an impossible remainder".into());
    }
    if input.contains('=') && !bytes.len().is_multiple_of(4) {
        return Err("padded base64 must use complete quartets".into());
    }
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    let mut index = 0;
    while index < bytes.len() {
        let remaining = bytes.len() - index;
        let take = remaining.min(4);
        let mut values = [0u8; 4];
        let mut padding = 0;
        for offset in 0..take {
            let byte = bytes[index + offset];
            if byte == b'=' {
                padding += 1;
                values[offset] = 0;
                if index + take != bytes.len() {
                    return Err("base64 padding is not terminal".into());
                }
            } else if padding != 0 {
                return Err("base64 data follows padding".into());
            } else {
                values[offset] = url_safe_value(byte).ok_or_else(|| {
                    format!("invalid base64 character at byte {}", index + offset)
                })?;
            }
        }
        if padding > 2 {
            return Err("base64 has too much padding".into());
        }
        if take < 4 && padding != 0 {
            return Err("padding requires a complete quartet".into());
        }
        if take >= 2 {
            out.push((values[0] << 2) | (values[1] >> 4));
        }
        if take >= 3 && bytes[index + 2] != b'=' {
            out.push((values[1] << 4) | (values[2] >> 2));
        }
        if take == 4 && bytes[index + 3] != b'=' {
            out.push((values[2] << 6) | values[3]);
        }
        index += take;
    }
    Ok(out)
}

fn url_safe_value(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'-' | b'+' => Some(62),
        b'_' | b'/' => Some(63),
        _ => None,
    }
}

/// Decode an unpadded literal under the exact URL-safe `-_` alphabet.
///
/// The proven four-character quantum template reads every input character
/// through the proven alphabet and has no padding branch, so a literal that
/// still contains `=`, or the standard `+/` characters, cannot be decoded
/// without guessing and is refused instead.
pub(crate) fn decode_url_safe_strict(input: &str, limit: usize) -> Result<Vec<u8>, String> {
    if let Some(&byte) = input
        .as_bytes()
        .iter()
        .find(|byte| url_safe_strict_value(**byte).is_none())
    {
        if matches!(byte, b'+' | b'/') {
            return Err(
                "consumer decoder requires the URL-safe -_ alphabet; standard +/ rejected".into(),
            );
        }
        if byte == b'=' {
            return Err(
                "consumer decoder requires the URL-safe -_ alphabet; padding rejected".into(),
            );
        }
        return Err("consumer decoder contains a character outside the URL-safe alphabet".into());
    }
    let expected = input.len() / 4 * 3
        + match input.len() % 4 {
            2 => 1,
            3 => 2,
            _ => 0,
        };
    if expected > limit {
        return Err(format!("decoded artifact exceeds configured limit {limit}"));
    }
    decode_url_safe(input)
}

/// The exact eight-bit value of one URL-safe base64 character.
fn url_safe_strict_value(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'-' => Some(62),
        b'_' => Some(63),
        _ => None,
    }
}

fn decode_standard(input: &str, limit: usize) -> Result<Vec<u8>, String> {
    if input.contains('-') || input.contains('_') {
        return Err(
            "consumer decoder requires the standard +/ alphabet; URL-safe -_ rejected".into(),
        );
    }
    if input
        .as_bytes()
        .iter()
        .any(|b| !STANDARD_ALPHABET.contains(b) && *b != b'=')
    {
        return Err("consumer decoder contains a character outside the standard alphabet".into());
    }
    let bytes = input.as_bytes();
    if bytes.len() % 4 == 1 {
        return Err("base64 length cannot leave one trailing sextet".into());
    }
    let padding = bytes.iter().position(|b| *b == b'=');
    if let Some(padding) = padding {
        if !bytes.len().is_multiple_of(4)
            || bytes.len() - padding > 2
            || bytes[padding..].iter().any(|b| *b != b'=')
            || (bytes.len() - padding == 2 && bytes[padding + 1] != b'=')
            || padding < 2
        {
            return Err("base64 padding is malformed".into());
        }
    }
    // Keep this historical estimate.  The strict consumer's acceptance at
    // configured boundaries is part of the runtime-config contract.
    let expected = bytes.len() / 4 * 3
        + match bytes.len() % 4 {
            0 => padding.map_or(0, |p| 3 - (bytes.len() - p)),
            2 => 1,
            3 => 2,
            _ => 0,
        };
    if expected > limit {
        return Err(format!("decoded artifact exceeds configured limit {limit}"));
    }
    let mut out = Vec::new();
    out.try_reserve_exact(expected)
        .map_err(|_| "decoded artifact allocation failed".to_string())?;
    let mut i = 0;
    while i < bytes.len() {
        let remaining = bytes.len() - i;
        let take = remaining.min(4);
        if take == 1 {
            return Err("base64 consumer has a trailing single character".into());
        }
        let mut value = [0u8; 4];
        for j in 0..take {
            value[j] = match bytes[i + j] {
                b'A'..=b'Z' => bytes[i + j] - b'A',
                b'a'..=b'z' => bytes[i + j] - b'a' + 26,
                b'0'..=b'9' => bytes[i + j] - b'0' + 52,
                b'+' => 62,
                b'/' => 63,
                b'=' => 0,
                _ => unreachable!(),
            };
        }
        if take >= 2 {
            out.push((value[0] << 2) | (value[1] >> 4));
        }
        if take >= 3 && bytes[i + 2] != b'=' {
            out.push((value[1] << 4) | (value[2] >> 2));
        }
        if take == 4 && bytes[i + 3] != b'=' {
            out.push((value[2] << 6) | value[3]);
        }
        if bytes[i..i + take].contains(&b'=') && i + take != bytes.len() {
            return Err("base64 padding is not terminal".into());
        }
        i += take;
    }
    Ok(out)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ValidationStatus {
    Valid,
    Invalid(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ModuleLimits {
    pub(crate) max_bytes: usize,
    /// Per-table materialization threshold used only by the detailed boundary
    /// projection.  It is not a global entry budget and never rejects a module.
    pub(crate) max_entries: usize,
}

impl Default for ModuleLimits {
    fn default() -> Self {
        Self {
            max_bytes: 8 * 1024 * 1024,
            max_entries: 100_000,
        }
    }
}

impl ModuleLimits {
    /// Legacy callers had no byte cap of their own.  Derive the inspection
    /// bound from the finite input they already hold instead of applying the
    /// opt-in default cap to their historical acceptance.
    pub(crate) fn for_input(bytes: &[u8]) -> Self {
        Self {
            max_bytes: bytes.len(),
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AbiKind {
    Function,
    Table,
    Memory,
    Global,
    Tag,
    FunctionExact,
}

impl AbiKind {
    fn from_type_ref(ty: &TypeRef) -> Self {
        match ty {
            TypeRef::Func(_) => Self::Function,
            TypeRef::Table(_) => Self::Table,
            TypeRef::Memory(_) => Self::Memory,
            TypeRef::Global(_) => Self::Global,
            TypeRef::Tag(_) => Self::Tag,
            TypeRef::FuncExact(_) => Self::FunctionExact,
        }
    }

    fn from_external(kind: ExternalKind) -> Self {
        match kind {
            ExternalKind::Func => Self::Function,
            ExternalKind::Table => Self::Table,
            ExternalKind::Memory => Self::Memory,
            ExternalKind::Global => Self::Global,
            ExternalKind::Tag => Self::Tag,
            ExternalKind::FuncExact => Self::FunctionExact,
        }
    }

    /// Runtime-config historically projected only these export kinds.
    pub(crate) fn is_config_export(self) -> bool {
        matches!(self, Self::Function | Self::Table | Self::Memory)
    }

    /// Layout projected every old external kind except the newer exact
    /// function reference kind.
    pub(crate) fn is_layout_export(self) -> bool {
        matches!(
            self,
            Self::Function | Self::Table | Self::Memory | Self::Global | Self::Tag
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AbiImport {
    pub(crate) namespace: String,
    pub(crate) name: String,
    pub(crate) kind: AbiKind,
    /// True only for the legacy `Imports::Single` group.  Config's historical
    /// memory flag deliberately ignored compact import groups.
    pub(crate) single: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AbiExport {
    pub(crate) name: String,
    pub(crate) kind: AbiKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AbiErrorKind {
    Payload,
    Import,
    Export,
}

/// The first neutral ABI read error, plus the vector lengths at that point.
/// Config flattened these errors; properties rejected them; layout preserved
/// the partial vectors and its fixed error text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AbiError {
    pub(crate) kind: AbiErrorKind,
    pub(crate) imports: usize,
    pub(crate) exports: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct AbiSummary {
    pub(crate) imports: Vec<AbiImport>,
    pub(crate) exports: Vec<AbiExport>,
    pub(crate) single_imported_memory: bool,
    pub(crate) defined_memory: bool,
    pub(crate) first_error: Option<AbiError>,
}

impl AbiSummary {
    fn record_error(&mut self, kind: AbiErrorKind) {
        if self.first_error.is_none() {
            self.first_error = Some(AbiError {
                kind,
                imports: self.imports.len(),
                exports: self.exports.len(),
            });
        }
    }

    fn observe(&mut self, payload: &Payload<'_>) {
        match payload {
            Payload::ImportSection(reader) => self.observe_imports(reader),
            Payload::MemorySection(reader) => {
                self.defined_memory |= reader.count() > 0;
            }
            Payload::ExportSection(reader) => self.observe_exports(reader),
            _ => {}
        }
    }

    fn observe_imports(&mut self, reader: &wasmparser::ImportSectionReader<'_>) {
        for group in reader.clone() {
            match group {
                Ok(Imports::Single(_, import)) => {
                    if matches!(import.ty, TypeRef::Memory(_)) {
                        self.single_imported_memory = true;
                    }
                    self.imports.push(AbiImport {
                        namespace: import.module.into(),
                        name: import.name.into(),
                        kind: AbiKind::from_type_ref(&import.ty),
                        single: true,
                    });
                }
                Ok(Imports::Compact1 { module, items }) => {
                    for item in items.into_iter().flatten() {
                        self.imports.push(AbiImport {
                            namespace: module.into(),
                            name: item.name.into(),
                            kind: AbiKind::from_type_ref(&item.ty),
                            single: false,
                        });
                    }
                }
                Ok(Imports::Compact2 { module, ty, names }) => {
                    for name in names.into_iter().flatten() {
                        self.imports.push(AbiImport {
                            namespace: module.into(),
                            name: name.into(),
                            kind: AbiKind::from_type_ref(&ty),
                            single: false,
                        });
                    }
                }
                Err(_) => {
                    self.record_error(AbiErrorKind::Import);
                    return;
                }
            }
        }
    }

    fn observe_exports(&mut self, reader: &wasmparser::ExportSectionReader<'_>) {
        for item in reader.clone() {
            match item {
                Ok(export) => self.exports.push(AbiExport {
                    name: export.name.to_owned(),
                    kind: AbiKind::from_external(export.kind),
                }),
                Err(_) => {
                    self.record_error(AbiErrorKind::Export);
                    return;
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ModuleSummary {
    pub(crate) validation: ValidationStatus,
    pub(crate) abi: AbiSummary,
    /// Error from the optional detailed projection.  `None` when no detailed
    /// consumer was requested or when it completed successfully.
    pub(crate) detailed_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ModuleError {
    ByteLimit { actual: usize, limit: usize },
}

impl std::fmt::Display for ModuleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ByteLimit { actual, limit } => {
                write!(
                    f,
                    "WASM bytes exceed configured limit {limit} (got {actual})"
                )
            }
        }
    }
}

/// Validate, then walk one shared `Parser` payload stream and project the
/// neutral ABI facts.  No detailed section is read, so a GC type, malformed
/// body, or malformed table can never change this projection.
pub(crate) fn inspect_module(
    bytes: &[u8],
    limits: ModuleLimits,
) -> Result<ModuleSummary, ModuleError> {
    inspect_module_detailed(bytes, limits, |_| Ok(()))
}

/// Validate, then walk one shared `Parser` payload stream.  The `detailed`
/// callback receives each payload after validation succeeds and may project
/// the rich boundary record from cloned readers.  Its error is recorded in
/// `detailed_error` and never contaminates the neutral ABI projection.
pub(crate) fn inspect_module_detailed<'a, F>(
    bytes: &'a [u8],
    limits: ModuleLimits,
    mut detailed: F,
) -> Result<ModuleSummary, ModuleError>
where
    F: FnMut(&Payload<'a>) -> Result<(), String>,
{
    if bytes.len() > limits.max_bytes {
        return Err(ModuleError::ByteLimit {
            actual: bytes.len(),
            limit: limits.max_bytes,
        });
    }
    let validation = match Validator::new().validate_all(bytes) {
        Ok(_) => ValidationStatus::Valid,
        Err(error) => ValidationStatus::Invalid(error.to_string()),
    };
    // The historical detailed boundary parser ran only after successful
    // validation.  Keeping that ordering also avoids materializing table
    // entries for validation-invalid inputs.
    let project_details = matches!(validation, ValidationStatus::Valid);
    let mut abi = AbiSummary::default();
    let mut detailed_error = None;
    for payload in Parser::new(0).parse_all(bytes) {
        match payload {
            Ok(payload) => {
                if project_details && detailed_error.is_none() {
                    if let Err(error) = detailed(&payload) {
                        detailed_error = Some(error);
                    }
                }
                abi.observe(&payload);
            }
            Err(error) => {
                if project_details && detailed_error.is_none() {
                    detailed_error = Some(error.to_string());
                }
                abi.record_error(AbiErrorKind::Payload);
                break;
            }
        }
    }
    Ok(ModuleSummary {
        validation,
        abi,
        detailed_error,
    })
}

#[cfg(test)]
pub(crate) mod test_support {
    pub(crate) fn module(sections: &[Vec<u8>]) -> Vec<u8> {
        let mut bytes = b"\0asm\x01\0\0\0".to_vec();
        for section in sections {
            bytes.extend_from_slice(section);
        }
        bytes
    }

    pub(crate) fn section(id: u8, body: &[u8]) -> Vec<u8> {
        let mut bytes = vec![id];
        push_uleb(&mut bytes, body.len());
        bytes.extend_from_slice(body);
        bytes
    }

    fn push_uleb(out: &mut Vec<u8>, mut value: usize) {
        loop {
            let byte = (value & 0x7f) as u8;
            value >>= 7;
            if value == 0 {
                out.push(byte);
                break;
            }
            out.push(byte | 0x80);
        }
    }

    /// Valid module: one defined memory and memory export `m` (no GC types).
    pub(crate) fn memory_export_module() -> Vec<u8> {
        module(&[
            section(5, &[0x01, 0x00, 0x01]),
            section(7, &[0x01, 0x01, b'm', 0x02, 0x00]),
        ])
    }

    /// Valid module: one GC struct type, one defined memory, memory export `m`.
    pub(crate) fn gc_memory_export_module() -> Vec<u8> {
        module(&[
            section(1, &[0x01, 0x5f, 0x00]),
            section(5, &[0x01, 0x00, 0x01]),
            section(7, &[0x01, 0x01, b'm', 0x02, 0x00]),
        ])
    }

    /// Valid module: a GC struct type plus a normal function import and export.
    /// Only the detailed boundary parser rejects the GC type; the ABI is sound.
    pub(crate) fn gc_import_export_module() -> Vec<u8> {
        module(&[
            section(1, &[0x02, 0x5f, 0x00, 0x60, 0x00, 0x00]),
            section(2, &[0x01, 0x03, b'e', b'n', b'v', 0x01, b'f', 0x00, 0x01]),
            section(7, &[0x01, 0x03, b'r', b'u', b'n', 0x00, 0x00]),
        ])
    }

    /// Structurally parseable but validator-invalid: export function zero with
    /// no function section or import.
    pub(crate) fn invalid_export_module() -> Vec<u8> {
        module(&[section(7, &[0x01, 0x01, b'x', 0x00, 0x00])])
    }

    /// Valid memory export after a malformed type section.  Properties ignored
    /// the type section; config and layout reject the module at validation.
    pub(crate) fn malformed_type_memory_export_module() -> Vec<u8> {
        module(&[
            section(1, &[0x01, 0xff]),
            section(5, &[0x01, 0x00, 0x01]),
            section(7, &[0x01, 0x01, b'm', 0x02, 0x00]),
        ])
    }

    /// Malformed function body; properties historically ignored the body.
    pub(crate) fn malformed_body_module() -> Vec<u8> {
        module(&[
            section(1, &[0x01, 0x60, 0x00, 0x00]),
            section(3, &[0x01, 0x00]),
            section(10, &[0x01, 0x03, 0x00, 0xff, 0x0b]),
        ])
    }

    /// Malformed table entry followed by a memory export; properties ignored
    /// the table section.
    pub(crate) fn malformed_table_memory_export_module() -> Vec<u8> {
        module(&[
            section(4, &[0x01, 0x70, 0x05, 0x01]),
            section(5, &[0x01, 0x00, 0x01]),
            section(7, &[0x01, 0x01, b'm', 0x02, 0x00]),
        ])
    }

    /// Compact-2 memory import.  Config's memory flag intentionally ignores
    /// compact groups, even though the exported memory is the imported one.
    pub(crate) fn compact2_memory_import_module() -> Vec<u8> {
        module(&[
            section(
                2,
                &[
                    0x01, 0x03, b'e', b'n', b'v', 0x00, 0x7e, 0x02, 0x00, 0x01, 0x01, 0x01, b'm',
                ],
            ),
            section(7, &[0x01, 0x01, b'm', 0x02, 0x00]),
        ])
    }

    /// Compact-1 group with two function imports.
    pub(crate) fn compact1_imports_module() -> Vec<u8> {
        module(&[
            section(1, &[0x01, 0x60, 0x00, 0x00]),
            section(
                2,
                &[
                    0x01, 0x03, b'e', b'n', b'v', 0x00, 0x7f, 0x02, 0x01, b'a', 0x00, 0x00, 0x01,
                    b'b', 0x00, 0x00,
                ],
            ),
        ])
    }

    /// Compact-1 group promises two items but carries one; old simple readers
    /// flattened the inner error and still reported the readable import/export.
    pub(crate) fn compact1_truncated_with_export_module() -> Vec<u8> {
        module(&[
            section(
                2,
                &[
                    0x01, 0x03, b'e', b'n', b'v', 0x00, 0x7f, 0x02, 0x01, b'a', 0x00, 0x00,
                ],
            ),
            section(7, &[0x01, 0x01, b'x', 0x00, 0x00]),
        ])
    }

    /// One valid single import followed by a truncated import group.
    pub(crate) fn partial_import_then_error_module() -> Vec<u8> {
        module(&[section(
            2,
            &[0x02, 0x03, b'e', b'n', b'v', 0x01, b'a', 0x00, 0x00, 0x05],
        )])
    }

    /// A valid import followed by a truncated export item.
    pub(crate) fn truncated_export_after_import_module() -> Vec<u8> {
        module(&[
            section(2, &[0x01, 0x03, b'e', b'n', b'v', 0x01, b'f', 0x00, 0x00]),
            section(7, &[0x01, 0x05, b'x']),
        ])
    }

    /// Truncated first import group.
    pub(crate) fn truncated_import_module() -> Vec<u8> {
        module(&[section(2, &[0x01, 0x05])])
    }

    /// Valid module larger than the opt-in 8 MiB default because of a large
    /// custom section.  Old unbounded callers accepted it.
    pub(crate) fn large_custom_section_module() -> Vec<u8> {
        let mut custom = vec![0u8];
        custom.resize(1 + 8 * 1024 * 1024, 0);
        module(&[
            section(5, &[0x01, 0x00, 0x01]),
            section(7, &[0x01, 0x01, b'm', 0x02, 0x00]),
            section(0, &custom),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::*;
    use super::*;

    #[test]
    fn pins_url_safe_decoder_edges() {
        assert_eq!(
            decode_base64("SGVsbG8", Base64Policy::UrlSafe).unwrap(),
            b"Hello"
        );
        assert_eq!(
            decode_base64("SGVsbG8=", Base64Policy::UrlSafe).unwrap(),
            b"Hello"
        );
        assert_eq!(
            decode_base64("-_8", Base64Policy::UrlSafe).unwrap(),
            [251, 255]
        );
        assert!(decode_base64("", Base64Policy::UrlSafe).is_err());
        assert!(decode_base64("A", Base64Policy::UrlSafe).is_err());
        assert!(decode_base64("YQ=", Base64Policy::UrlSafe).is_err());
        assert!(decode_base64("Y Q", Base64Policy::UrlSafe).is_err());
        assert!(decode_base64("YQ@", Base64Policy::UrlSafe).is_err());
    }

    #[test]
    fn pins_strict_decoder_policy_and_limit_boundary() {
        let padded = decode_base64("AAA=", Base64Policy::Standard { limit: 5 }).unwrap();
        assert_eq!(padded, [0, 0]);
        assert!(decode_base64("AAA=", Base64Policy::Standard { limit: 4 }).is_err());
        assert!(decode_base64("-_8", Base64Policy::Standard { limit: 16 }).is_err());
        assert!(decode_base64("Y Q=", Base64Policy::Standard { limit: 16 }).is_err());
        assert_eq!(
            decode_base64("", Base64Policy::Standard { limit: 0 }).unwrap(),
            Vec::<u8>::new()
        );
        assert!(decode_base64("AAAA", Base64Policy::Standard { limit: 2 }).is_err());
    }

    #[test]
    fn pins_url_safe_strict_decoder_edges() {
        assert_eq!(decode_url_safe_strict("-_8", 16).unwrap(), [251, 255]);
        assert_eq!(decode_url_safe_strict("SGVsbG8", 16).unwrap(), b"Hello");
        assert_eq!(decode_url_safe_strict("SGVsbG8", 5).unwrap(), b"Hello");
        assert!(decode_url_safe_strict("SGVsbG8", 4).is_err());
        assert!(decode_url_safe_strict("+/8", 16).is_err());
        assert!(decode_url_safe_strict("YQ==", 16).is_err());
        assert!(decode_url_safe_strict("YQ=", 16).is_err());
        assert!(decode_url_safe_strict("Y Q", 16).is_err());
        assert!(decode_url_safe_strict("YQ@", 16).is_err());
        assert!(decode_url_safe_strict("A", 16).is_err());
    }

    #[test]
    fn abi_projection_ignores_detailed_unsupported_types() {
        let bytes = gc_import_export_module();
        // Emulate the old detailed boundary parser rejecting a GC type.
        let mut observed_type_section = false;
        let summary =
            inspect_module_detailed(
                &bytes,
                ModuleLimits::for_input(&bytes),
                |payload| match payload {
                    Payload::TypeSection(_) => {
                        observed_type_section = true;
                        Err("gc proposal not supported".into())
                    }
                    _ => Ok(()),
                },
            )
            .expect("in bounds");
        assert!(observed_type_section);
        assert_eq!(summary.validation, ValidationStatus::Valid);
        assert_eq!(
            summary.detailed_error.as_deref(),
            Some("gc proposal not supported")
        );
        assert_eq!(summary.abi.imports.len(), 1);
        assert_eq!(summary.abi.imports[0].namespace, "env");
        assert_eq!(summary.abi.imports[0].name, "f");
        assert_eq!(summary.abi.exports.len(), 1);
        assert_eq!(summary.abi.exports[0].name, "run");
    }

    #[test]
    fn abi_records_partial_vectors_at_first_import_error() {
        let bytes = partial_import_then_error_module();
        let summary = inspect_module(&bytes, ModuleLimits::for_input(&bytes)).expect("in bounds");
        assert_eq!(summary.abi.imports.len(), 1);
        assert_eq!(summary.abi.imports[0].name, "a");
        let error = summary.abi.first_error.expect("import error");
        assert_eq!(error.kind, AbiErrorKind::Import);
        assert_eq!(error.imports, 1);
        assert_eq!(error.exports, 0);
    }

    #[test]
    fn compact_import_memory_flag_is_single_import_only() {
        let bytes = compact2_memory_import_module();
        let summary = inspect_module(&bytes, ModuleLimits::for_input(&bytes)).expect("in bounds");
        assert_eq!(summary.abi.imports[0].name, "m");
        assert!(!summary.abi.imports[0].single);
        assert!(!summary.abi.single_imported_memory);
        assert!(!summary.abi.defined_memory);
    }

    #[test]
    fn default_byte_cap_is_explicit_opt_in_only() {
        let bytes = large_custom_section_module();
        assert!(bytes.len() > 8 * 1024 * 1024);
        assert!(matches!(
            inspect_module(&bytes, ModuleLimits::default()),
            Err(ModuleError::ByteLimit { .. })
        ));
        let summary = inspect_module(&bytes, ModuleLimits::for_input(&bytes)).expect("in bounds");
        assert_eq!(summary.validation, ValidationStatus::Valid);
        assert_eq!(summary.abi.exports[0].name, "m");
        assert!(summary.abi.defined_memory);
    }
}
