use super::*;

/// A proof that the source contains exactly the admitted helper and decoder.
/// The source digest and both spans make this proof unusable for a different
/// source or a different helper/decoder occurrence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmStringProof {
    pub(super) source_sha256: [u8; 32],
    pub(super) helper_span: Span,
    pub(super) decoder_span: Span,
    pub(super) decoder_proof: DecoderProof,
    pub(super) dependencies: DependencyAssumptions,
}

/// Why a source cannot be used as proof for the admitted helper.
///
/// Every failure leaves the call unfolded; the variants are deliberately
/// specific enough to distinguish a malformed span/parse from a helper shape,
/// a decoder template mismatch, untrusted dependencies, and source drift.
#[derive(Debug, Error)]
pub enum VmStringProofError {
    #[error("source span is outside the supplied source")]
    InvalidSpan,
    #[error("source does not parse: {0}")]
    Parse(String),
    #[error("helper span is not the exact admitted vmDecodeString function")]
    HelperShape,
    #[error("manual base64 decoder was not proven at the supplied span")]
    DecoderNotProven,
    #[error("dependency assumptions are not all trusted data-only bindings")]
    UntrustedDependencies,
    #[error("proof source digest does not match the supplied source")]
    SourceMismatch,
}

impl VmStringProof {
    /// Reconstruct the evaluator proof from a compiler-attested, validated
    /// source-free plan.  No source digest is asserted here: the plan records
    /// the structural proof result, while artifact integrity/version checks
    /// establish the trust boundary for this runtime operation.
    pub(crate) fn from_compiled_plan(
        destination_parameter_index: Option<usize>,
        accepts_padding: bool,
        url_safe: bool,
        dependencies: DependencyAssumptions,
    ) -> Result<Self, VmStringProofError> {
        if !dependencies.is_trusted() {
            return Err(VmStringProofError::UntrustedDependencies);
        }
        let (rule, alphabet, template) = if url_safe {
            (
                "url-safe-base64-decoder-v1",
                URL_SAFE_ALPHABET,
                "four-char URL-safe base64 quantum with bounded tail stores",
            )
        } else {
            (
                "standard-base64-decoder-v1",
                STANDARD_ALPHABET,
                "four-char standard base64 quantum with bounded tail stores",
            )
        };
        Ok(Self {
            source_sha256: [0; 32],
            helper_span: Span::new(0, 0),
            decoder_span: Span::new(0, 0),
            decoder_proof: DecoderProof {
                rule,
                alphabet,
                destination_parameter_index,
                accepts_padding,
                template,
                evidence: crate::kernel::decoder_proof::DecoderProofEvidence {
                    input_quantum: 4,
                    output_quantum: 3,
                    byte_formulas: "(Y<<2)|(k>>4), ((k&15)<<4)|(H>>2), ((H&3)<<6)|h",
                },
            },
            dependencies,
        })
    }

    /// Prove an exact helper/decoder pair in `source`.  This is structural and
    /// non-executing; identifier spelling alone cannot satisfy the helper
    /// matcher.
    pub fn prove(
        source: &str,
        helper_span: Span,
        decoder_span: Span,
        dependencies: DependencyAssumptions,
    ) -> Result<Self, VmStringProofError> {
        validate_span(source, helper_span)?;
        validate_span(source, decoder_span)?;
        if !dependencies.is_trusted() {
            return Err(VmStringProofError::UntrustedDependencies);
        }
        prove_helper_shape(source, helper_span).map_err(|_| VmStringProofError::HelperShape)?;
        let decoder_proof = prove_runtime_decoder(source, decoder_span)
            .ok_or(VmStringProofError::DecoderNotProven)?;
        Ok(Self {
            source_sha256: digest(source),
            helper_span,
            decoder_span,
            decoder_proof,
            dependencies,
        })
    }

    /// Construct a proof after the two exact structural matchers have each
    /// accepted their component.  This is private to the integration layer:
    /// callers must obtain `decoder_proof` from `prove_decoder_candidate` and
    /// separately prove the helper candidate before using this constructor.
    pub(super) fn from_proven_components(
        source: &str,
        helper_span: Span,
        decoder_span: Span,
        decoder_proof: DecoderProof,
        dependencies: DependencyAssumptions,
    ) -> Result<Self, VmStringProofError> {
        validate_span(source, helper_span)?;
        validate_span(source, decoder_span)?;
        if !dependencies.is_trusted() {
            return Err(VmStringProofError::UntrustedDependencies);
        }
        Ok(Self {
            source_sha256: digest(source),
            helper_span,
            decoder_span,
            decoder_proof,
            dependencies,
        })
    }

    /// Re-check the source identity and proof at the original spans.
    pub fn verify_source(&self, source: &str) -> Result<(), VmStringProofError> {
        if digest(source) != self.source_sha256 {
            return Err(VmStringProofError::SourceMismatch);
        }
        Self::prove(
            source,
            self.helper_span,
            self.decoder_span,
            self.dependencies,
        )
        .map(|_| ())
    }

    /// The exact decoder template accepted at `decoder_span`. This is evidence
    /// for downstream planning, not authorization to skip source verification.
    pub fn decoder_proof(&self) -> DecoderProof {
        self.decoder_proof
    }

    /// Stable identity of the exact source bytes used to construct this proof.
    /// Callers can retain it with captured VM values and reject reuse across
    /// source artifacts before entering a future integration layer.
    pub fn source_sha256(&self) -> [u8; 32] {
        self.source_sha256
    }

    /// The explicit table/cache/decoder/fromCharCode assumptions this proof
    /// was constructed with.
    pub fn dependencies(&self) -> DependencyAssumptions {
        self.dependencies
    }
}

/// Run the exact helper matcher once for an integration candidate.
pub(super) fn prove_helper_candidate(
    source: &str,
    helper_span: Span,
) -> Result<(), VmStringProofError> {
    prove_helper_shape(source, helper_span)
}

/// Run the exact decoder matcher once for an integration candidate.
pub(super) fn prove_decoder_candidate(source: &str, decoder_span: Span) -> Option<DecoderProof> {
    prove_runtime_decoder(source, decoder_span)
}
pub(super) fn evaluate_helper_source(
    source: &str,
    helper_span: Span,
) -> Result<(), VmStringProofError> {
    let allocator = oxc_allocator::Allocator::default();
    let mut parsed = Parser::new(&allocator, source, SourceType::unambiguous())
        .with_options(ParseOptions {
            preserve_parens: false,
            ..ParseOptions::default()
        })
        .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return Err(VmStringProofError::Parse(format!(
            "{} parse errors",
            parsed.errors.len()
        )));
    }
    let (scoping, nodes) = analysis::analyze(&parsed.program);
    let Some(target) = nodes.iter().find_map(|node| {
        matches!(node.kind(), AstKind::Function(function) if function.span == helper_span)
            .then(|| node.kind())
    }) else {
        return Err(VmStringProofError::HelperShape);
    };
    let AstKind::Function(target_function) = target else {
        return Err(VmStringProofError::HelperShape);
    };
    if target_function.params.items.len() != 7 || target_function.body.is_none() {
        return Err(VmStringProofError::HelperShape);
    }
    if target_function.r#async || target_function.generator {
        return Err(VmStringProofError::HelperShape);
    }
    let expected = canonical_helper_source();
    let expected_allocator = oxc_allocator::Allocator::default();
    let mut expected_parsed =
        Parser::new(&expected_allocator, &expected, SourceType::unambiguous())
            .with_options(ParseOptions {
                preserve_parens: false,
                ..ParseOptions::default()
            })
            .parse();
    if expected_parsed.panicked || !expected_parsed.errors.is_empty() {
        return Err(VmStringProofError::HelperShape);
    }
    let (_, expected_nodes) = analysis::analyze(&expected_parsed.program);
    let expected_function = expected_nodes
        .iter()
        .find_map(|node| match node.kind() {
            AstKind::Function(function) => Some(function),
            _ => None,
        })
        .ok_or(VmStringProofError::HelperShape)?;
    let expected_span = expected_function.span;
    let mut normalizer = HelperNormalizer::new(helper_span, &scoping);
    normalizer.visit_program(&mut parsed.program);
    if !normalizer.valid || normalizer.bindings.len() > MAX_ROLE_NAMES.len() {
        return Err(VmStringProofError::HelperShape);
    }
    let expected_scoping = analysis::analyze(&expected_parsed.program).0;
    let mut expected_normalizer = HelperNormalizer::new(expected_span, &expected_scoping);
    expected_normalizer.visit_program(&mut expected_parsed.program);
    if !expected_normalizer.valid {
        return Err(VmStringProofError::HelperShape);
    }
    let (_, nodes_after) = analysis::analyze(&parsed.program);
    let target_after = nodes_after
        .iter()
        .find_map(|node| match node.kind() {
            AstKind::Function(function) if function.span == helper_span => Some(function),
            _ => None,
        })
        .ok_or(VmStringProofError::HelperShape)?;
    let (_, expected_nodes_after) = analysis::analyze(&expected_parsed.program);
    let expected_after = expected_nodes_after
        .iter()
        .find_map(|node| match node.kind() {
            AstKind::Function(function) if function.span == expected_span => Some(function),
            _ => None,
        })
        .ok_or(VmStringProofError::HelperShape)?;
    if !target_after.content_eq(expected_after) {
        return Err(VmStringProofError::HelperShape);
    }
    Ok(())
}

pub(super) fn prove_helper_shape(
    source: &str,
    helper_span: Span,
) -> Result<(), VmStringProofError> {
    evaluate_helper_source(source, helper_span)
}

pub(super) fn canonical_helper_source() -> String {
    "function vmDecodeString(encodedString, keyIndex, stringTable, cache, decodeBase64, fromCharCode, computeOnly) {\n\
      if (!computeOnly) {\n\
        var cacheKey = encodedString + \",\" + keyIndex;\n\
        var cachedString = cache[cacheKey];\n\
        if (typeof cachedString !== \"undefined\") { return cachedString; }\n\
      }\n\
      var keyBytes = decodeBase64(stringTable[keyIndex]);\n\
      var encodedBytes = decodeBase64(encodedString);\n\
      var xorMask = keyBytes[0] + encodedBytes[0] & 255;\n\
      var decodedString = \"\";\n\
      for (var byteIndex = 1; byteIndex < keyBytes.length; ++byteIndex) {\n\
        decodedString += fromCharCode(encodedBytes[byteIndex] ^ keyBytes[byteIndex] ^ xorMask);\n\
      }\n\
      if (!computeOnly) { cache[cacheKey] = decodedString; }\n\
      return decodedString;\n\
    }"
    .into()
}

pub(super) struct HelperNormalizer<'a> {
    pub(super) target: Span,
    pub(super) scoping: &'a oxc_semantic::Scoping,
    pub(super) active: bool,
    pub(super) valid: bool,
    pub(super) bindings: Vec<(oxc_semantic::SymbolId, Ident<'static>)>,
}

impl<'a> HelperNormalizer<'a> {
    pub(super) fn new(target: Span, scoping: &'a oxc_semantic::Scoping) -> Self {
        Self {
            target,
            scoping,
            active: false,
            valid: true,
            bindings: Vec::new(),
        }
    }

    pub(super) fn name_for(&mut self, symbol: oxc_semantic::SymbolId) -> Option<Ident<'static>> {
        if let Some((_, name)) = self.bindings.iter().find(|(found, _)| *found == symbol) {
            return Some(*name);
        }
        let name = MAX_ROLE_NAMES
            .get(self.bindings.len())
            .copied()
            .map(Ident::new_const)?;
        self.bindings.push((symbol, name));
        Some(name)
    }
}

impl<'a, 'b> VisitMut<'b> for HelperNormalizer<'a> {
    fn visit_function(&mut self, function: &mut Function<'b>, flags: ScopeFlags) {
        if function.span == self.target {
            self.active = true;
            walk_mut::walk_function(self, function, flags);
            self.active = false;
        } else if self.active {
            self.valid = false;
        } else {
            walk_mut::walk_function(self, function, flags);
        }
    }

    fn visit_binding_identifier(&mut self, identifier: &mut BindingIdentifier<'b>) {
        if self.active {
            let symbol = binding_symbol(identifier, self.scoping);
            if let Some(name) = symbol.and_then(|symbol| self.name_for(symbol)) {
                identifier.name = name;
            } else {
                self.valid = false;
            }
        }
        walk_mut::walk_binding_identifier(self, identifier);
    }

    fn visit_identifier_reference(&mut self, identifier: &mut IdentifierReference<'b>) {
        if self.active {
            let Some(symbol) = analysis::identifier_symbol(identifier, self.scoping) else {
                if identifier.name != "undefined" {
                    self.valid = false;
                }
                walk_mut::walk_identifier_reference(self, identifier);
                return;
            };
            if let Some((_, name)) = self.bindings.iter().find(|(found, _)| *found == symbol) {
                identifier.name = *name;
            } else {
                self.valid = false;
            }
        }
        walk_mut::walk_identifier_reference(self, identifier);
    }
}

pub(super) fn binding_symbol(
    identifier: &BindingIdentifier<'_>,
    scoping: &oxc_semantic::Scoping,
) -> Option<oxc_semantic::SymbolId> {
    scoping
        .symbol_ids()
        .find(|symbol| scoping.symbol_span(*symbol) == identifier.span)
}

pub(super) fn validate_span(source: &str, span: Span) -> Result<(), VmStringProofError> {
    (span.start <= span.end
        && usize::try_from(span.end)
            .ok()
            .is_some_and(|end| end <= source.len()))
    .then_some(())
    .ok_or(VmStringProofError::InvalidSpan)
}

pub(super) fn digest(source: &str) -> [u8; 32] {
    Sha256::digest(source.as_bytes()).into()
}
