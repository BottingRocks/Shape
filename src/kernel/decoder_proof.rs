//! A narrow, non-executing recognizer for the base64 bytecode decoder.
//!
//! The decoder body is checked as a complete AST template. Before comparing
//! it, local identifiers are renamed from Oxc's resolved declaration identity;
//! source spelling never determines a match.  The alphabet is one more
//! dimension of the template: the standard `+/` alphabet and the URL-safe `-_`
//! alphabet are proved by the same loop and byte formulas, and the resulting
//! [`DecoderProof`] names the alphabet it accepted.

use std::collections::{BTreeMap, BTreeSet};

use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstKind};
use oxc_ast_visit::{walk, walk_mut, Visit, VisitMut};
use oxc_parser::{ParseOptions, Parser};
use oxc_semantic::{ReferenceFlags, Scoping, SymbolId};
use oxc_span::{ContentEq, GetSpan, Ident, SourceType, Span};
use oxc_syntax::operator::{AssignmentOperator, BinaryOperator, UnaryOperator};

/// The standard base64 alphabet.
pub const STANDARD_ALPHABET: &str =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
/// The URL-safe base64 alphabet.
pub const URL_SAFE_ALPHABET: &str =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
const BUILTINS: [&str; 4] = ["Array", "Math", "String", "Uint8Array"];

/// The alphabet-specific half of a decoder proof.
#[derive(Debug, Clone, Copy)]
struct AlphabetClaim {
    alphabet: &'static str,
    rule: &'static str,
    template: &'static str,
}

const STANDARD_CLAIM: AlphabetClaim = AlphabetClaim {
    alphabet: STANDARD_ALPHABET,
    rule: "standard-base64-decoder-v1",
    template: "four-char standard base64 quantum with bounded tail stores",
};

const URL_SAFE_CLAIM: AlphabetClaim = AlphabetClaim {
    alphabet: URL_SAFE_ALPHABET,
    rule: "url-safe-base64-decoder-v1",
    template: "four-char URL-safe base64 quantum with bounded tail stores",
};
const ROLE_NAMES: [&str; 24] = [
    "r0", "r1", "r2", "r3", "r4", "r5", "r6", "r7", "r8", "r9", "r10", "r11", "r12", "r13", "r14",
    "r15", "r16", "r17", "r18", "r19", "r20", "r21", "r22", "r23",
];

/// Evidence for the fixed decoder geometry established by
/// [`prove_standard_decoder`] and [`prove_runtime_decoder`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecoderProofEvidence {
    /// Source characters consumed by each loop turn.
    pub input_quantum: usize,
    /// Output bytes reserved by each loop turn.
    pub output_quantum: usize,
    /// The three fixed base64 byte formulas.
    pub byte_formulas: &'static str,
}

/// A proof that a function is the complete base64 decoder template.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecoderProof {
    /// Stable rule identifier for downstream evidence consumers.
    pub rule: &'static str,
    /// The exact base64 alphabet proved by the matcher.
    pub alphabet: &'static str,
    /// The caller-provided destination parameter, when present in the template.
    pub destination_parameter_index: Option<usize>,
    /// The manual template does not prove padded-input equivalence.
    pub accepts_padding: bool,
    /// Human-readable name of the complete matched template.
    pub template: &'static str,
    /// Bounded arithmetic evidence for the matched loop.
    pub evidence: DecoderProofEvidence,
}

/// Prove the complete standard base64 decoder at exactly `function_span`.
pub fn prove_standard_decoder(source: &str, function_span: Span) -> Option<DecoderProof> {
    prove_decoder(source, function_span, &[STANDARD_CLAIM])
}

/// Prove the complete base64 decoder at exactly `function_span`, accepting
/// either the standard `+/` alphabet or the URL-safe `-_` alphabet.  The
/// returned [`DecoderProof::alphabet`] is the alphabet the template matched.
pub fn prove_runtime_decoder(source: &str, function_span: Span) -> Option<DecoderProof> {
    prove_decoder(source, function_span, &[STANDARD_CLAIM, URL_SAFE_CLAIM])
}

fn prove_decoder(
    source: &str,
    function_span: Span,
    claims: &[AlphabetClaim],
) -> Option<DecoderProof> {
    // Native cleanup may deliberately retain an immutable alias graph when a
    // dynamic native mutation makes inlining unsafe. Recover only exact
    // decoder aliases for this proof; the cleanup barrier remains unchanged.
    if let Some((expanded, expanded_span)) = expand_decoder_aliases(source, function_span) {
        return prove_decoder_inner(&expanded, expanded_span, claims);
    }
    prove_decoder_inner(source, function_span, claims)
}

fn prove_decoder_inner(
    source: &str,
    function_span: Span,
    claims: &[AlphabetClaim],
) -> Option<DecoderProof> {
    let allocator = Allocator::default();
    let mut parsed = Parser::new(&allocator, source, SourceType::unambiguous())
        .with_options(ParseOptions {
            preserve_parens: false,
            ..ParseOptions::default()
        })
        .parse();
    if parsed.panicked || !parsed.errors.is_empty() || source_has_dynamic_hazards(&parsed.program) {
        return None;
    }
    let (scoping, nodes) = crate::analysis::analyze(&parsed.program);
    if !native_intrinsics_are_stable(&parsed.program, &scoping, &nodes) {
        return None;
    }
    let mut variant = None;
    let mut alias = None;
    let found = with_function(&parsed.program, function_span, |target| {
        variant = Some(variant_of(target));
        alias = allocator_symbol(target, &scoping);
        true
    });
    let (Some(variant), Some(alias)) = (variant, alias) else {
        return None;
    };
    if variant.inline_allocator != alias.is_none() {
        return None;
    }
    if !found
        || !with_function(&parsed.program, function_span, |target| {
            unshadowed_builtins(target, &scoping)
        })
        || !alias_is_safe(&parsed.program, alias, &scoping)
    {
        return None;
    }

    let mut normalizer = Normalizer::new(function_span, &scoping, alias);
    normalizer.visit_program(&mut parsed.program);
    if !normalizer.valid {
        return None;
    }
    for claim in claims {
        for uncurried in [false, true] {
            let template = template_source(uncurried, variant, claim.alphabet);
            if matches_template(
                &mut parsed.program,
                &allocator,
                function_span,
                &template,
                variant.inline_allocator,
            ) {
                return Some(DecoderProof {
                    rule: claim.rule,
                    alphabet: claim.alphabet,
                    destination_parameter_index: Some(1),
                    accepts_padding: false,
                    template: claim.template,
                    evidence: DecoderProofEvidence {
                        input_quantum: 4,
                        output_quantum: 3,
                        byte_formulas: "(Y<<2)|(k>>4), ((k&15)<<4)|(H>>2), ((H&3)<<6)|h",
                    },
                });
            }
        }
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DecoderAlias {
    Builtin(&'static str),
    NativeMember(&'static str, &'static str),
    StringMethod(&'static str),
    PartialUncurried,
    UncurriedStringMethod(&'static str),
    Allocator,
}

/// Rewrite exact immutable aliases used by the retained decoder helpers into
/// the canonical proof spelling. This is a proof-input transformation only;
/// the actual decoder source and its dependency closure remain unchanged.
fn expand_decoder_aliases(source: &str, function_span: Span) -> Option<(String, Span)> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::unambiguous())
        .with_options(ParseOptions {
            preserve_parens: false,
            ..ParseOptions::default()
        })
        .parse();
    if parsed.panicked || !parsed.errors.is_empty() || source_has_dynamic_hazards(&parsed.program) {
        return None;
    }
    let (scoping, nodes) = crate::analysis::analyze(&parsed.program);
    let facts = decoder_alias_facts(&parsed.program, &scoping, &nodes, function_span.start);
    if facts.is_empty() {
        return None;
    }
    let decoder_symbol = function_symbol(&parsed.program, function_span);
    if !decoder_alias_use_is_safe(&scoping, &nodes, function_span, decoder_symbol, &facts) {
        return None;
    }

    struct Collector<'a> {
        target: Span,
        scoping: &'a Scoping,
        facts: &'a BTreeMap<SymbolId, DecoderAlias>,
        edits: Vec<crate::analysis::Edit>,
    }
    impl<'a, 'b> Visit<'b> for Collector<'a> {
        fn visit_call_expression(&mut self, call: &CallExpression<'b>) {
            if call.span.start >= self.target.start && call.span.end <= self.target.end {
                if let Expression::Identifier(identifier) = &call.callee {
                    if let Some(symbol) = identifier_symbol(identifier, self.scoping) {
                        if let Some(DecoderAlias::UncurriedStringMethod(method)) =
                            self.facts.get(&symbol)
                        {
                            self.edits.push(crate::analysis::Edit::replace(
                                identifier.span,
                                format!("String.prototype.{method}.call"),
                            ));
                        } else if let Some(DecoderAlias::NativeMember(object, property)) =
                            self.facts.get(&symbol)
                        {
                            self.edits.push(crate::analysis::Edit::replace(
                                identifier.span,
                                format!("{object}.{property}"),
                            ));
                        }
                    }
                }
            }
            walk::walk_call_expression(self, call);
        }

        fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'b>) {
            if identifier.span.start >= self.target.start
                && identifier.span.end <= self.target.end
                && identifier_symbol(identifier, self.scoping)
                    .and_then(|symbol| self.facts.get(&symbol))
                    .is_some_and(|fact| matches!(fact, DecoderAlias::Allocator))
            {
                self.edits.push(crate::analysis::Edit::replace(
                    identifier.span,
                    "(typeof Uint8Array === \"function\" ? Uint8Array : Array)",
                ));
            }
            walk::walk_identifier_reference(self, identifier);
        }
    }

    let mut collector = Collector {
        target: function_span,
        scoping: &scoping,
        facts: &facts,
        edits: Vec::new(),
    };
    collector.visit_program(&parsed.program);
    if collector.edits.is_empty() {
        return None;
    }
    let delta: i64 = collector
        .edits
        .iter()
        .map(|edit| edit.replacement.len() as i64 - i64::from(edit.end - edit.start))
        .sum();
    let applied = crate::analysis::apply_edits(source, collector.edits);
    if !applied.rejected.is_empty() {
        return None;
    }
    let end = i64::from(function_span.end) + delta;
    if end >= i64::from(function_span.start) && end <= i64::from(u32::MAX) {
        Some((applied.output, Span::new(function_span.start, end as u32)))
    } else {
        None
    }
}

fn decoder_alias_facts(
    program: &Program<'_>,
    scoping: &Scoping,
    nodes: &oxc_semantic::AstNodes<'_>,
    capture_start: u32,
) -> BTreeMap<SymbolId, DecoderAlias> {
    let mut values = BTreeMap::new();
    for (node_id, node) in nodes.iter_enumerated() {
        if let AstKind::VariableDeclaration(declaration) = node.kind() {
            if declaration.kind != VariableDeclarationKind::Var
                || declaration.span.end > capture_start
                || !decoder_alias_declaration_context_is_unconditional(nodes, node_id)
            {
                continue;
            }
            for declarator in &declaration.declarations {
                if let (Some(symbol), Some(init)) = (
                    binding_symbol(&declarator.id, scoping),
                    declarator.init.as_ref(),
                ) {
                    values.insert(symbol, init);
                }
            }
        }
    }

    let mut facts = BTreeMap::new();
    for _ in 0..=values.len() {
        let before = facts.len();
        for (&symbol, expression) in &values {
            if facts.contains_key(&symbol) || !stable_alias(symbol, scoping) {
                continue;
            }
            if !decoder_alias_initializer_order_is_safe(expression, &values, scoping, nodes) {
                continue;
            }
            if let Some(fact) = classify_decoder_alias(expression, scoping, &facts) {
                if !decoder_alias_binding_is_safe(program, symbol, scoping) {
                    continue;
                }
                facts.insert(symbol, fact);
            }
        }
        if facts.len() == before {
            break;
        }
    }
    facts
}

/// Symbols declared before `before` that the retained-decoder alias proof
/// resolves to the native `charAt` (directly, curried, or uncurried). This is
/// used only to name decoder locals; it never rewrites source semantics.
pub(crate) fn proven_char_at_aliases(
    program: &Program<'_>,
    scoping: &Scoping,
    nodes: &oxc_semantic::AstNodes<'_>,
    before: u32,
) -> BTreeSet<SymbolId> {
    decoder_alias_facts(program, scoping, nodes, before)
        .into_iter()
        .filter_map(|(symbol, alias)| match alias {
            DecoderAlias::StringMethod("charAt")
            | DecoderAlias::UncurriedStringMethod("charAt") => Some(symbol),
            _ => None,
        })
        .collect()
}

fn decoder_alias_binding_is_safe(
    program: &Program<'_>,
    alias: SymbolId,
    scoping: &Scoping,
) -> bool {
    struct Guard<'a> {
        alias: SymbolId,
        scoping: &'a Scoping,
        bad: bool,
    }
    impl<'a, 'b> Visit<'b> for Guard<'a> {
        fn visit_assignment_expression(&mut self, expression: &AssignmentExpression<'b>) {
            self.bad |= target_is_external(&expression.left, Some(self.alias), self.scoping);
            walk::walk_assignment_expression(self, expression);
        }

        fn visit_update_expression(&mut self, expression: &UpdateExpression<'b>) {
            self.bad |=
                simple_target_is_external(&expression.argument, Some(self.alias), self.scoping);
            walk::walk_update_expression(self, expression);
        }
    }
    let mut guard = Guard {
        alias,
        scoping,
        bad: false,
    };
    guard.visit_program(program);
    !guard.bad
}

fn decoder_alias_use_is_safe(
    scoping: &Scoping,
    nodes: &oxc_semantic::AstNodes<'_>,
    target: Span,
    decoder_symbol: Option<SymbolId>,
    facts: &BTreeMap<SymbolId, DecoderAlias>,
) -> bool {
    let mut values = BTreeMap::new();
    for (node_id, node) in nodes.iter_enumerated() {
        let AstKind::VariableDeclaration(declaration) = node.kind() else {
            continue;
        };
        if declaration.kind != VariableDeclarationKind::Var
            || declaration.span.end > target.start
            || !decoder_alias_declaration_context_is_unconditional(nodes, node_id)
        {
            continue;
        }
        for declarator in &declaration.declarations {
            if let (Some(symbol), Some(init)) = (
                binding_symbol(&declarator.id, scoping),
                declarator.init.as_ref(),
            ) {
                values.insert(symbol, init);
            }
        }
    }
    let snapshot_initializers = nodes
        .iter_enumerated()
        .filter_map(|(node_id, node)| {
            let AstKind::VariableDeclaration(declaration) = node.kind() else {
                return None;
            };
            (declaration.kind == VariableDeclarationKind::Var
                && declaration.span.end <= target.start
                && decoder_alias_declaration_context_is_unconditional(nodes, node_id))
            .then(|| {
                declaration
                    .declarations
                    .iter()
                    .filter_map(|declarator| declarator.init.as_ref())
                    .filter(|init| {
                        matches!(
                            init,
                            Expression::ArrayExpression(_) | Expression::ObjectExpression(_)
                        )
                    })
                    .map(|init| init.span())
                    .collect::<Vec<_>>()
            })
        })
        .flatten()
        .collect::<Vec<_>>();

    let reference_span = |reference: &oxc_semantic::Reference| match nodes.kind(reference.node_id())
    {
        AstKind::IdentifierReference(identifier) => Some(identifier.span),
        _ => None,
    };
    let inside = |outer: Span, inner: Span| inner.start >= outer.start && inner.end <= outer.end;

    // Start with facts actually consumed by the decoder, then close over the
    // immutable alias initializers. Unused facts must not make an otherwise
    // valid decoder fail merely because they exist elsewhere in the program.
    let mut closure = facts
        .keys()
        .copied()
        .filter(|symbol| {
            scoping.get_resolved_references(*symbol).any(|reference| {
                reference.flags().contains(ReferenceFlags::Read)
                    && reference_span(reference).is_some_and(|span| inside(target, span))
            })
        })
        .collect::<BTreeSet<_>>();
    loop {
        let before = closure.len();
        for symbol in closure.clone() {
            let Some(init) = values.get(&symbol) else {
                return false;
            };
            for dependency in values.keys().copied() {
                if scoping
                    .get_resolved_references(dependency)
                    .any(|reference| {
                        reference.flags().contains(ReferenceFlags::Read)
                            && reference_span(reference)
                                .is_some_and(|span| inside(init.span(), span))
                    })
                {
                    closure.insert(dependency);
                }
            }
            for reference in scoping.get_resolved_references(symbol) {
                if !reference.flags().contains(ReferenceFlags::Read) {
                    continue;
                }
                let Some(span) = reference_span(reference) else {
                    return false;
                };
                if inside(target, span)
                    || snapshot_initializers
                        .iter()
                        .any(|initializer| inside(*initializer, span))
                {
                    continue;
                }
                if let Some(owner) = values.iter().find_map(|(owner, init)| {
                    (inside(init.span(), span)
                        && matches!(
                            init,
                            Expression::Identifier(_)
                                | Expression::StaticMemberExpression(_)
                                | Expression::ComputedMemberExpression(_)
                        ))
                    .then_some(*owner)
                }) {
                    closure.insert(owner);
                }
            }
        }
        if closure.len() == before {
            break;
        }
    }
    // A proven alias may be read by its dependency initializer or by the
    // decoder itself. Any other read escapes the proof boundary: an unknown
    // call/control-flow path could observe or mutate the retained value.
    if let Some(decoder_symbol) = decoder_symbol {
        let required_initialization_end = closure
            .iter()
            .filter_map(|symbol| values.get(symbol))
            .map(|initializer| initializer.span().end)
            .max();
        if required_initialization_end.is_some_and(|end| {
            scoping
                .get_resolved_references(decoder_symbol)
                .any(|reference| {
                    reference.flags().contains(ReferenceFlags::Read)
                        && reference_span(reference)
                            .is_some_and(|span| span.start < end && !inside(target, span))
                })
        }) {
            return false;
        }
    }

    closure.iter().all(|symbol| {
        scoping.get_resolved_references(*symbol).all(|reference| {
            if !reference.flags().contains(ReferenceFlags::Read) {
                return true;
            }
            let Some(span) = reference_span(reference) else {
                return false;
            };
            let allowed = inside(target, span)
                || values.values().any(|init| inside(init.span(), span))
                || snapshot_initializers
                    .iter()
                    .any(|initializer| inside(*initializer, span))
                || reference_is_non_escaping(nodes, reference);
            allowed
        })
    })
}

fn decoder_alias_initializer_order_is_safe(
    initializer: &Expression<'_>,
    values: &BTreeMap<SymbolId, &Expression<'_>>,
    scoping: &Scoping,
    nodes: &oxc_semantic::AstNodes<'_>,
) -> bool {
    values.iter().all(|(dependency, dependency_initializer)| {
        scoping
            .get_resolved_references(*dependency)
            .all(|reference| {
                let Some(span) = identifier_reference_span(nodes, reference) else {
                    return true;
                };
                if span.start < initializer.span().start || span.end > initializer.span().end {
                    return true;
                }
                dependency_initializer.span().end <= span.start
            })
    })
}

fn identifier_reference_span(
    nodes: &oxc_semantic::AstNodes<'_>,
    reference: &oxc_semantic::Reference,
) -> Option<Span> {
    match nodes.kind(reference.node_id()) {
        AstKind::IdentifierReference(identifier) => Some(identifier.span),
        _ => None,
    }
}

fn decoder_alias_declaration_context_is_unconditional(
    nodes: &oxc_semantic::AstNodes<'_>,
    node_id: oxc_semantic::NodeId,
) -> bool {
    nodes.ancestor_kinds(node_id).all(|kind| {
        matches!(
            kind,
            AstKind::Program(_)
                | AstKind::Function(_)
                | AstKind::FunctionBody(_)
                | AstKind::ArrowFunctionExpression(_)
                | AstKind::CallExpression(_)
                | AstKind::ExpressionStatement(_)
                | AstKind::ParenthesizedExpression(_)
        )
    })
}

fn reference_is_non_escaping(
    nodes: &oxc_semantic::AstNodes<'_>,
    reference: &oxc_semantic::Reference,
) -> bool {
    let span = match nodes.kind(reference.node_id()) {
        AstKind::IdentifierReference(identifier) => identifier.span,
        _ => return false,
    };
    match nodes.parent_kind(reference.node_id()) {
        AstKind::StaticMemberExpression(member) => member.object.span().contains_inclusive(span),
        AstKind::ComputedMemberExpression(member) => member.object.span().contains_inclusive(span),
        AstKind::CallExpression(call) => call.callee.span().contains_inclusive(span),
        AstKind::NewExpression(new_expression) => {
            new_expression.callee.span().contains_inclusive(span)
        }
        _ => false,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct NativePath {
    root: &'static str,
    members: Vec<String>,
}

fn native_root(name: &str) -> Option<&'static str> {
    match name {
        "Array" | "Function" | "Math" | "Object" | "Reflect" | "String" | "Uint8Array"
        | "globalThis" | "self" | "window" => Some(match name {
            "Array" => "Array",
            "Function" => "Function",
            "Math" => "Math",
            "Object" => "Object",
            "Reflect" => "Reflect",
            "String" => "String",
            "Uint8Array" => "Uint8Array",
            "globalThis" => "globalThis",
            "self" => "self",
            "window" => "window",
            _ => unreachable!(),
        }),
        _ => None,
    }
}

fn native_path(
    expression: &Expression<'_>,
    aliases: &BTreeMap<SymbolId, NativePath>,
    scoping: &Scoping,
) -> Option<NativePath> {
    match expression {
        Expression::Identifier(identifier) => match identifier_symbol(identifier, scoping) {
            Some(symbol) => aliases.get(&symbol).cloned(),
            None => native_root(identifier.name.as_str()).map(|root| NativePath {
                root,
                members: Vec::new(),
            }),
        },
        Expression::StaticMemberExpression(member) => {
            let mut path = native_path(&member.object, aliases, scoping)?;
            path.members.push(member.property.name.to_string());
            Some(path)
        }
        _ => None,
    }
}

fn native_rooted_expression(
    expression: &Expression<'_>,
    aliases: &BTreeMap<SymbolId, NativePath>,
    scoping: &Scoping,
) -> bool {
    match expression {
        Expression::Identifier(_) | Expression::StaticMemberExpression(_) => {
            native_path(expression, aliases, scoping).is_some()
        }
        Expression::ComputedMemberExpression(member) => {
            native_rooted_expression(&member.object, aliases, scoping)
        }
        _ => false,
    }
}

fn native_alias_paths(
    scoping: &Scoping,
    nodes: &oxc_semantic::AstNodes<'_>,
) -> BTreeMap<SymbolId, NativePath> {
    let mut values = BTreeMap::new();
    for node in nodes.iter() {
        let AstKind::VariableDeclarator(declarator) = node.kind() else {
            continue;
        };
        if let (Some(symbol), Some(init)) = (
            binding_symbol(&declarator.id, scoping),
            declarator.init.as_ref(),
        ) {
            values.insert(symbol, init);
        }
    }

    let mut aliases = BTreeMap::new();
    for _ in 0..=values.len() {
        let before = aliases.len();
        for (&symbol, expression) in &values {
            if aliases.contains_key(&symbol) {
                continue;
            }
            if let Some(path) = native_path(expression, &aliases, scoping) {
                aliases.insert(symbol, path);
            }
        }
        if aliases.len() == before {
            break;
        }
    }
    aliases
}

fn native_intrinsics_are_stable(
    program: &Program<'_>,
    scoping: &Scoping,
    nodes: &oxc_semantic::AstNodes<'_>,
) -> bool {
    let aliases = native_alias_paths(scoping, nodes);
    let mut guard = NativeMutationGuard {
        aliases: &aliases,
        scoping,
        bad: false,
    };
    guard.visit_program(program);
    !guard.bad
}

struct NativeMutationGuard<'a> {
    aliases: &'a BTreeMap<SymbolId, NativePath>,
    scoping: &'a Scoping,
    bad: bool,
}

impl<'a, 'b> Visit<'b> for NativeMutationGuard<'a> {
    fn visit_assignment_expression(&mut self, expression: &AssignmentExpression<'b>) {
        self.bad |=
            native_assignment_target_is_external(&expression.left, self.aliases, self.scoping);
        walk::walk_assignment_expression(self, expression);
    }

    fn visit_update_expression(&mut self, expression: &UpdateExpression<'b>) {
        self.bad |=
            native_simple_target_is_external(&expression.argument, self.aliases, self.scoping);
        walk::walk_update_expression(self, expression);
    }

    fn visit_unary_expression(&mut self, expression: &UnaryExpression<'b>) {
        if expression.operator == UnaryOperator::Delete {
            self.bad |= native_rooted_expression(&expression.argument, self.aliases, self.scoping);
        }
        walk::walk_unary_expression(self, expression);
    }

    fn visit_call_expression(&mut self, expression: &CallExpression<'b>) {
        if let Some(path) = native_path(&expression.callee, self.aliases, self.scoping) {
            if matches!(path.root, "Object" | "Reflect")
                && path.members.iter().any(|member| {
                    matches!(
                        member.as_str(),
                        "assign"
                            | "defineProperty"
                            | "defineProperties"
                            | "preventExtensions"
                            | "seal"
                            | "setPrototypeOf"
                            | "freeze"
                    )
                })
                && expression.arguments.iter().any(|argument| {
                    argument.as_expression().is_some_and(|argument| {
                        native_rooted_expression(argument, self.aliases, self.scoping)
                    })
                })
            {
                self.bad = true;
            }
        }
        walk::walk_call_expression(self, expression);
    }
}

fn native_assignment_target_is_external(
    target: &AssignmentTarget<'_>,
    aliases: &BTreeMap<SymbolId, NativePath>,
    scoping: &Scoping,
) -> bool {
    match target {
        AssignmentTarget::AssignmentTargetIdentifier(identifier) => {
            native_identifier_is_external(identifier, aliases, scoping)
        }
        AssignmentTarget::StaticMemberExpression(member) => {
            native_rooted_expression(&member.object, aliases, scoping)
        }
        AssignmentTarget::ComputedMemberExpression(member) => {
            native_rooted_expression(&member.object, aliases, scoping)
        }
        _ => false,
    }
}

fn native_simple_target_is_external(
    target: &SimpleAssignmentTarget<'_>,
    aliases: &BTreeMap<SymbolId, NativePath>,
    scoping: &Scoping,
) -> bool {
    match target {
        SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier) => {
            native_identifier_is_external(identifier, aliases, scoping)
        }
        SimpleAssignmentTarget::StaticMemberExpression(member) => {
            native_rooted_expression(&member.object, aliases, scoping)
        }
        SimpleAssignmentTarget::ComputedMemberExpression(member) => {
            native_rooted_expression(&member.object, aliases, scoping)
        }
        _ => false,
    }
}

fn native_identifier_is_external(
    identifier: &IdentifierReference<'_>,
    aliases: &BTreeMap<SymbolId, NativePath>,
    scoping: &Scoping,
) -> bool {
    identifier_symbol(identifier, scoping)
        .and_then(|symbol| aliases.get(&symbol))
        .is_some()
        || identifier_symbol(identifier, scoping).is_none()
            && native_root(identifier.name.as_str()).is_some()
}

fn stable_alias(symbol: SymbolId, scoping: &Scoping) -> bool {
    scoping.symbol_declarations(symbol).count() == 1
        && !scoping
            .get_resolved_references(symbol)
            .any(|reference| reference.flags().contains(ReferenceFlags::Write))
}

fn classify_decoder_alias(
    expression: &Expression<'_>,
    scoping: &Scoping,
    facts: &BTreeMap<SymbolId, DecoderAlias>,
) -> Option<DecoderAlias> {
    match expression {
        Expression::Identifier(identifier) => match identifier_symbol(identifier, scoping) {
            Some(symbol) => facts.get(&symbol).copied(),
            None if matches!(
                identifier.name.as_str(),
                "Array" | "Math" | "Object" | "String" | "Uint8Array"
            ) =>
            {
                Some(DecoderAlias::Builtin(match identifier.name.as_str() {
                    "Array" => "Array",
                    "Math" => "Math",
                    "Uint8Array" => "Uint8Array",
                    "Object" => "Object",
                    "String" => "String",
                    _ => unreachable!(),
                }))
            }
            _ => None,
        },
        Expression::StaticMemberExpression(member) => {
            let property = match member.property.name.as_str() {
                "bind" => "bind",
                "call" => "call",
                "indexOf" => "indexOf",
                "charAt" => "charAt",
                "floor" => "floor",
                "fromCharCode" => "fromCharCode",
                _ => return None,
            };
            if matches!(property, "indexOf" | "charAt")
                && matches!(&member.object, Expression::StringLiteral(_))
            {
                return Some(DecoderAlias::StringMethod(if property == "indexOf" {
                    "indexOf"
                } else {
                    "charAt"
                }));
            }
            if let Expression::Identifier(object) = &member.object {
                if identifier_symbol(object, scoping).is_none()
                    && matches!(
                        (object.name.as_str(), property),
                        ("Math", "floor") | ("String", "fromCharCode")
                    )
                {
                    return Some(DecoderAlias::NativeMember(
                        if object.name == "Math" {
                            "Math"
                        } else {
                            "String"
                        },
                        if property == "floor" {
                            "floor"
                        } else {
                            "fromCharCode"
                        },
                    ));
                }
            }
            let Expression::Identifier(object) = &member.object else {
                return None;
            };
            let object_symbol = identifier_symbol(object, scoping)?;
            match facts.get(&object_symbol)? {
                DecoderAlias::Builtin("Object") if matches!(property, "bind" | "call") => {
                    Some(DecoderAlias::NativeMember("Object", property))
                }
                DecoderAlias::Builtin("Math") if property == "floor" => {
                    Some(DecoderAlias::NativeMember("Math", "floor"))
                }
                DecoderAlias::Builtin("String") if property == "fromCharCode" => {
                    Some(DecoderAlias::NativeMember("String", "fromCharCode"))
                }
                _ => None,
            }
        }
        Expression::ConditionalExpression(conditional)
            if is_allocator_condition(&conditional.test, scoping)
                && is_global_or_alias(&conditional.consequent, "Uint8Array", scoping, facts)
                && is_global_or_alias(&conditional.alternate, "Array", scoping, facts) =>
        {
            Some(DecoderAlias::Allocator)
        }
        Expression::CallExpression(call) => {
            if let Expression::Identifier(callee) = &call.callee {
                let symbol = identifier_symbol(callee, scoping)?;
                if matches!(facts.get(&symbol), Some(DecoderAlias::PartialUncurried))
                    && call.arguments.len() == 1
                {
                    let argument = call.arguments.first()?.as_expression()?;
                    return match classify_decoder_alias(argument, scoping, facts) {
                        Some(DecoderAlias::StringMethod(method)) => {
                            Some(DecoderAlias::UncurriedStringMethod(method))
                        }
                        _ => None,
                    };
                }
            }
            let Expression::StaticMemberExpression(member) = &call.callee else {
                return None;
            };
            if member.property.name != "bind" || call.arguments.len() != 2 {
                return None;
            }
            let Expression::Identifier(object) = &member.object else {
                return None;
            };
            let object_symbol = identifier_symbol(object, scoping)?;
            let first = call.arguments[0].as_expression()?;
            let second = call.arguments[1].as_expression()?;
            let (Expression::Identifier(first), Expression::Identifier(second)) = (first, second)
            else {
                return None;
            };
            let first_symbol = identifier_symbol(first, scoping)?;
            let second_symbol = identifier_symbol(second, scoping)?;
            (second_symbol == object_symbol
                && matches!(
                    facts.get(&object_symbol),
                    Some(DecoderAlias::NativeMember("Object", "call"))
                )
                && matches!(
                    facts.get(&first_symbol),
                    Some(DecoderAlias::NativeMember("Object", "bind"))
                ))
            .then_some(DecoderAlias::PartialUncurried)
        }
        _ => None,
    }
}

fn is_allocator_condition(expression: &Expression<'_>, scoping: &Scoping) -> bool {
    let Expression::BinaryExpression(binary) = expression else {
        return false;
    };
    binary.operator == BinaryOperator::StrictEquality
        && matches!(&binary.left, Expression::UnaryExpression(unary)
            if unary.operator == UnaryOperator::Typeof
                && is_unresolved_global(&unary.argument, "Uint8Array", scoping))
        && matches!(&binary.right, Expression::StringLiteral(string) if string.value == "function")
}

fn is_global_or_alias(
    expression: &Expression<'_>,
    name: &str,
    scoping: &Scoping,
    facts: &BTreeMap<SymbolId, DecoderAlias>,
) -> bool {
    match expression {
        Expression::Identifier(identifier) => match identifier_symbol(identifier, scoping) {
            Some(symbol) => {
                matches!(facts.get(&symbol), Some(DecoderAlias::Builtin(found)) if *found == name)
            }
            None => identifier.name == name,
        },
        _ => false,
    }
}

#[derive(Clone, Copy)]
struct Variant {
    fast_path: bool,
    grouped_initials: bool,
    inline_allocator: bool,
}

fn function_symbol(program: &Program<'_>, span: Span) -> Option<SymbolId> {
    let mut symbol = None;
    with_function(program, span, |function| {
        symbol = function.id.as_ref().and_then(|id| id.symbol_id.get());
        true
    });
    symbol
}

fn with_function<'a, F>(program: &'a Program<'a>, span: Span, callback: F) -> bool
where
    F: FnMut(&Function<'a>) -> bool,
{
    struct Finder<F> {
        span: Span,
        callback: F,
        found: bool,
    }
    impl<'a, F> Visit<'a> for Finder<F>
    where
        F: FnMut(&Function<'a>) -> bool,
    {
        fn visit_function(
            &mut self,
            function: &Function<'a>,
            flags: oxc_syntax::scope::ScopeFlags,
        ) {
            if function.span == self.span {
                self.found = (self.callback)(function);
            }
            walk::walk_function(self, function, flags);
        }
    }
    let mut finder = Finder {
        span,
        callback,
        found: false,
    };
    finder.visit_program(program);
    finder.found
}

fn variant_of(function: &Function<'_>) -> Variant {
    let Some(body) = function.body.as_deref() else {
        return Variant {
            fast_path: false,
            grouped_initials: false,
            inline_allocator: false,
        };
    };
    let fast_path = matches!(body.statements.first(), Some(Statement::IfStatement(_)));
    let first = usize::from(fast_path);
    let grouped_initials = matches!(
        body.statements.get(first),
        Some(Statement::VariableDeclaration(declaration)) if declaration.declarations.len() == 2
    );
    let inline_allocator = body.statements.iter().any(|statement| {
        let Some(expression) = assignment_rhs(statement) else {
            return false;
        };
        let Expression::LogicalExpression(logical) = expression else {
            return false;
        };
        let Expression::NewExpression(new_expression) = &logical.right else {
            return false;
        };
        matches!(new_expression.callee, Expression::ConditionalExpression(_))
    });
    Variant {
        fast_path,
        grouped_initials,
        inline_allocator,
    }
}

fn assignment_rhs<'a>(statement: &'a Statement<'a>) -> Option<&'a Expression<'a>> {
    let Statement::ExpressionStatement(statement) = statement else {
        return None;
    };
    let Expression::AssignmentExpression(assignment) = &statement.expression else {
        return None;
    };
    (assignment.operator == AssignmentOperator::Assign).then_some(&assignment.right)
}

fn matches_template<'a>(
    source: &mut Program<'a>,
    source_allocator: &'a Allocator,
    source_span: Span,
    template_source: &str,
    inline: bool,
) -> bool {
    let allocator = Allocator::default();
    let mut parsed = Parser::new(&allocator, template_source, SourceType::unambiguous())
        .with_options(ParseOptions {
            preserve_parens: false,
            ..ParseOptions::default()
        })
        .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return false;
    }
    let (scoping, _) = crate::analysis::analyze(&parsed.program);
    let span = Span::new(
        template_source.find("function template").unwrap() as u32,
        template_source.len() as u32,
    );
    let mut alias = None;
    if !with_function(&parsed.program, span, |template| {
        alias = allocator_symbol(template, &scoping);
        true
    }) {
        return false;
    }
    let Some(alias) = alias.flatten() else {
        if !inline {
            return false;
        }
        let mut normalizer = Normalizer::new(span, &scoping, None);
        normalizer.visit_program(&mut parsed.program);
        if !normalizer.valid {
            return false;
        }
        // Declaration grouping is presentation-only. Compare both sides in the
        // same one-declaration-per-statement form the pipeline can emit.
        crate::transforms::deob::variable_declarations::split_program(
            &mut parsed.program,
            &allocator,
        );
        crate::transforms::deob::variable_declarations::split_program(source, source_allocator);
        return with_function(&parsed.program, span, |template| {
            let mut equal = false;
            with_function(source, source_span, |source| {
                equal = source.content_eq(template);
                true
            });
            equal
        });
    };
    let mut normalizer = Normalizer::new(span, &scoping, Some(alias));
    normalizer.visit_program(&mut parsed.program);
    if !normalizer.valid || inline {
        return false;
    }
    // Declaration grouping is presentation-only. Compare both sides in the
    // same one-declaration-per-statement form the pipeline can emit.
    crate::transforms::deob::variable_declarations::split_program(&mut parsed.program, &allocator);
    crate::transforms::deob::variable_declarations::split_program(source, source_allocator);
    with_function(&parsed.program, span, |template| {
        let mut equal = false;
        with_function(source, source_span, |source| {
            equal = source.content_eq(template);
            true
        });
        equal
    })
}

struct Normalizer<'a> {
    target: Span,
    scoping: &'a Scoping,
    alias: Option<SymbolId>,
    active: bool,
    valid: bool,
    bindings: Vec<(SymbolId, Ident<'static>)>,
}

impl<'a> Normalizer<'a> {
    fn new(target: Span, scoping: &'a Scoping, alias: Option<SymbolId>) -> Self {
        Self {
            target,
            scoping,
            alias,
            active: false,
            valid: true,
            bindings: Vec::new(),
        }
    }

    fn name_for(&mut self, symbol: SymbolId) -> Option<Ident<'static>> {
        if let Some((_, name)) = self.bindings.iter().find(|(found, _)| *found == symbol) {
            return Some(*name);
        }
        let name = ROLE_NAMES
            .get(self.bindings.len())
            .copied()
            .map(Ident::new_const)?;
        self.bindings.push((symbol, name));
        Some(name)
    }
}

impl<'a, 'b> VisitMut<'b> for Normalizer<'a> {
    fn visit_function(
        &mut self,
        function: &mut Function<'b>,
        flags: oxc_syntax::scope::ScopeFlags,
    ) {
        if function.span == self.target {
            self.active = true;
            walk_mut::walk_function(self, function, flags);
            self.active = false;
        } else {
            walk_mut::walk_function(self, function, flags);
        }
    }

    fn visit_binding_identifier(&mut self, identifier: &mut BindingIdentifier<'b>) {
        if self.active {
            let Some(symbol) = binding_identifier_symbol(identifier, self.scoping) else {
                self.valid = false;
                return;
            };
            if let Some(name) = self.name_for(symbol) {
                identifier.name = name;
            } else {
                self.valid = false;
            }
        }
        walk_mut::walk_binding_identifier(self, identifier);
    }

    fn visit_identifier_reference(&mut self, identifier: &mut IdentifierReference<'b>) {
        if self.active {
            let symbol = identifier_symbol(identifier, self.scoping);
            if let Some(symbol) = symbol {
                if let Some(name) = self
                    .bindings
                    .iter()
                    .find(|(found, _)| *found == symbol)
                    .map(|(_, name)| *name)
                {
                    identifier.name = name;
                } else if self.alias == Some(symbol) {
                    identifier.name = Ident::new_const("Alias");
                } else {
                    self.valid = false;
                }
            } else if !is_unshadowed_builtin(identifier, self.scoping) {
                self.valid = false;
            }
        }
        walk_mut::walk_identifier_reference(self, identifier);
    }
}

fn binding_symbol(pattern: &BindingPattern<'_>, scoping: &Scoping) -> Option<SymbolId> {
    let BindingPattern::BindingIdentifier(identifier) = pattern else {
        return None;
    };
    binding_identifier_symbol(identifier, scoping)
}

fn binding_identifier_symbol(
    identifier: &BindingIdentifier<'_>,
    scoping: &Scoping,
) -> Option<SymbolId> {
    scoping
        .symbol_ids()
        .find(|symbol| scoping.symbol_span(*symbol) == identifier.span)
}

fn identifier_symbol(identifier: &IdentifierReference<'_>, scoping: &Scoping) -> Option<SymbolId> {
    crate::analysis::identifier_symbol(identifier, scoping)
}

fn allocator_symbol(function: &Function<'_>, scoping: &Scoping) -> Option<Option<SymbolId>> {
    struct Finder<'a> {
        scoping: &'a Scoping,
        symbols: Vec<SymbolId>,
        invalid: bool,
    }
    impl<'a, 'b> Visit<'b> for Finder<'a> {
        fn visit_new_expression(&mut self, expression: &NewExpression<'b>) {
            if let Expression::Identifier(identifier) = &expression.callee {
                let Some(symbol) = identifier_symbol(identifier, self.scoping) else {
                    self.invalid = true;
                    walk::walk_new_expression(self, expression);
                    return;
                };
                if !self.symbols.contains(&symbol) {
                    self.symbols.push(symbol);
                }
            }
            walk::walk_new_expression(self, expression);
        }
    }
    let mut finder = Finder {
        scoping,
        symbols: Vec::new(),
        invalid: false,
    };
    finder.visit_function(function, oxc_syntax::scope::ScopeFlags::empty());
    if finder.invalid {
        return None;
    }
    match finder.symbols.as_slice() {
        [] => Some(None),
        [symbol] => Some(Some(*symbol)),
        _ => None,
    }
}

fn unshadowed_builtins(function: &Function<'_>, scoping: &Scoping) -> bool {
    struct Guard<'a> {
        scoping: &'a Scoping,
        bad: bool,
    }
    impl<'a, 'b> Visit<'b> for Guard<'a> {
        fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'b>) {
            if BUILTINS.contains(&identifier.name.as_str())
                && identifier_symbol(identifier, self.scoping).is_some()
            {
                self.bad = true;
            }
            walk::walk_identifier_reference(self, identifier);
        }
    }
    let mut guard = Guard {
        scoping,
        bad: false,
    };
    guard.visit_function(function, oxc_syntax::scope::ScopeFlags::empty());
    !guard.bad
}

fn is_unshadowed_builtin(identifier: &IdentifierReference<'_>, scoping: &Scoping) -> bool {
    identifier.reference_id.get().is_some()
        && identifier_symbol(identifier, scoping).is_none()
        && BUILTINS.contains(&identifier.name.as_str())
}

fn alias_is_safe(program: &Program<'_>, alias: Option<SymbolId>, scoping: &Scoping) -> bool {
    if let Some(alias) = alias {
        if scoping.symbol_declarations(alias).count() != 1
            || !has_exact_alias_declaration(program, alias, scoping)
        {
            return false;
        }
    }
    let mut guard = ExternalGuard {
        alias,
        scoping,
        bad: false,
        allowed_new: None,
    };
    guard.visit_program(program);
    !guard.bad
}

fn has_exact_alias_declaration(program: &Program<'_>, alias: SymbolId, scoping: &Scoping) -> bool {
    struct Finder<'a> {
        alias: SymbolId,
        scoping: &'a Scoping,
        found: bool,
        bad: bool,
    }
    impl<'a, 'b> Visit<'b> for Finder<'a> {
        fn visit_variable_declaration(&mut self, declaration: &VariableDeclaration<'b>) {
            for declarator in &declaration.declarations {
                if binding_symbol(&declarator.id, self.scoping) != Some(self.alias) {
                    continue;
                }
                let valid = declaration.kind == VariableDeclarationKind::Var
                    && declaration.declarations.len() == 1
                    && declarator
                        .init
                        .as_ref()
                        .is_some_and(|init| is_alias_initializer(init, self.scoping));
                self.bad |= self.found || !valid;
                self.found = true;
            }
            walk::walk_variable_declaration(self, declaration);
        }
    }
    let mut finder = Finder {
        alias,
        scoping,
        found: false,
        bad: false,
    };
    finder.visit_program(program);
    finder.found && !finder.bad
}

fn is_alias_initializer(expression: &Expression<'_>, scoping: &Scoping) -> bool {
    let Expression::ConditionalExpression(conditional) = expression else {
        return false;
    };
    typeof_function_test(&conditional.test, "Uint8Array", scoping)
        && is_unresolved_global(&conditional.consequent, "Uint8Array", scoping)
        && is_unresolved_global(&conditional.alternate, "Array", scoping)
}

fn typeof_function_test(expression: &Expression<'_>, global: &str, scoping: &Scoping) -> bool {
    let Expression::BinaryExpression(binary) = expression else {
        return false;
    };
    binary.operator == BinaryOperator::StrictEquality
        && matches!(&binary.left, Expression::UnaryExpression(unary)
            if unary.operator == UnaryOperator::Typeof && is_unresolved_global(&unary.argument, global, scoping))
        && matches!(&binary.right, Expression::StringLiteral(string) if string.value == "function")
}

fn is_unresolved_global(expression: &Expression<'_>, name: &str, scoping: &Scoping) -> bool {
    matches!(expression, Expression::Identifier(identifier)
        if identifier.name == name && identifier_symbol(identifier, scoping).is_none())
}

struct ExternalGuard<'a> {
    alias: Option<SymbolId>,
    scoping: &'a Scoping,
    bad: bool,
    allowed_new: Option<Span>,
}

impl<'a, 'b> Visit<'b> for ExternalGuard<'a> {
    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'b>) {
        if self
            .alias
            .is_some_and(|alias| Some(alias) == identifier_symbol(identifier, self.scoping))
            && self.allowed_new != Some(identifier.span)
        {
            self.bad = true;
        }
        walk::walk_identifier_reference(self, identifier);
    }

    fn visit_new_expression(&mut self, expression: &NewExpression<'b>) {
        let old = self.allowed_new;
        self.allowed_new = match &expression.callee {
            Expression::Identifier(identifier)
                if self.alias.is_some_and(|alias| {
                    Some(alias) == identifier_symbol(identifier, self.scoping)
                }) =>
            {
                Some(identifier.span)
            }
            _ => None,
        };
        walk::walk_new_expression(self, expression);
        self.allowed_new = old;
    }

    fn visit_assignment_expression(&mut self, expression: &AssignmentExpression<'b>) {
        if target_is_external(&expression.left, self.alias, self.scoping) {
            self.bad = true;
        }
        walk::walk_assignment_expression(self, expression);
    }

    fn visit_update_expression(&mut self, expression: &UpdateExpression<'b>) {
        if simple_target_is_external(&expression.argument, self.alias, self.scoping) {
            self.bad = true;
        }
        walk::walk_update_expression(self, expression);
    }
}

fn target_is_external(
    target: &AssignmentTarget<'_>,
    alias: Option<SymbolId>,
    scoping: &Scoping,
) -> bool {
    match target {
        AssignmentTarget::AssignmentTargetIdentifier(identifier) => {
            alias.is_some_and(|alias| Some(alias) == identifier_symbol(identifier, scoping))
                || unresolved_name(identifier, scoping)
        }
        AssignmentTarget::StaticMemberExpression(member) => {
            expression_is_external(&member.object, alias, scoping)
        }
        AssignmentTarget::ComputedMemberExpression(member) => {
            expression_is_external(&member.object, alias, scoping)
        }
        _ => false,
    }
}

fn simple_target_is_external(
    target: &SimpleAssignmentTarget<'_>,
    alias: Option<SymbolId>,
    scoping: &Scoping,
) -> bool {
    match target {
        SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier) => {
            alias.is_some_and(|alias| Some(alias) == identifier_symbol(identifier, scoping))
                || unresolved_name(identifier, scoping)
        }
        SimpleAssignmentTarget::StaticMemberExpression(member) => {
            expression_is_external(&member.object, alias, scoping)
        }
        SimpleAssignmentTarget::ComputedMemberExpression(member) => {
            expression_is_external(&member.object, alias, scoping)
        }
        _ => false,
    }
}

fn expression_is_external(
    expression: &Expression<'_>,
    alias: Option<SymbolId>,
    scoping: &Scoping,
) -> bool {
    match expression {
        Expression::Identifier(identifier) => {
            alias.is_some_and(|alias| Some(alias) == identifier_symbol(identifier, scoping))
                || unresolved_name(identifier, scoping)
        }
        Expression::StaticMemberExpression(member) => {
            expression_is_external(&member.object, alias, scoping)
        }
        Expression::ComputedMemberExpression(member) => {
            expression_is_external(&member.object, alias, scoping)
        }
        _ => false,
    }
}

fn unresolved_name(identifier: &IdentifierReference<'_>, scoping: &Scoping) -> bool {
    identifier_symbol(identifier, scoping).is_none() && BUILTINS.contains(&identifier.name.as_str())
}

struct HazardVisitor {
    dynamic_scope: bool,
}

impl<'a> Visit<'a> for HazardVisitor {
    fn visit_with_statement(&mut self, statement: &WithStatement<'a>) {
        self.dynamic_scope = true;
        walk::walk_with_statement(self, statement);
    }

    fn visit_call_expression(&mut self, expression: &CallExpression<'a>) {
        if matches!(&expression.callee, Expression::Identifier(identifier) if identifier.name == "eval")
        {
            self.dynamic_scope = true;
        }
        walk::walk_call_expression(self, expression);
    }
}

fn source_has_dynamic_hazards(program: &Program<'_>) -> bool {
    let mut visitor = HazardVisitor {
        dynamic_scope: false,
    };
    visitor.visit_program(program);
    visitor.dynamic_scope
}

const FAST_PATH: &str = r#"if (typeof Uint8Array === "function") {
  if (destination && destination.setFromBase64) {
    destination.setFromBase64(input);
    return destination;
  } else if (!destination && Uint8Array.fromBase64) {
    return Uint8Array.fromBase64(input);
  }
}
"#;

const DIRECT_LOOKUPS: &str = r#"    a = alpha.indexOf(input.charAt(inputCursor));
    b = alpha.indexOf(input.charAt(inputCursor + 1));
    c = alpha.indexOf(input.charAt(inputCursor + 2));
    d = alpha.indexOf(input.charAt(inputCursor + 3));
"#;

const UNCURRIED_LOOKUPS: &str = r#"    a = String.prototype.indexOf.call(alpha, String.prototype.charAt.call(input, inputCursor));
    b = String.prototype.indexOf.call(alpha, String.prototype.charAt.call(input, inputCursor + 1));
    c = String.prototype.indexOf.call(alpha, String.prototype.charAt.call(input, inputCursor + 2));
    d = String.prototype.indexOf.call(alpha, String.prototype.charAt.call(input, inputCursor + 3));
"#;

fn template_source(uncurried: bool, variant: Variant, alphabet: &str) -> String {
    let prefix = if variant.fast_path { FAST_PATH } else { "" };
    let initials = if variant.grouped_initials {
        format!("var alpha = \"{alphabet}\", length = input.length;")
    } else {
        format!("var alpha = \"{alphabet}\";\nvar length = input.length;")
    };
    let allocator = if variant.inline_allocator {
        "(typeof Uint8Array === \"function\" ? Uint8Array : Array)"
    } else {
        "Alias"
    };
    let lookups = if uncurried {
        UNCURRIED_LOOKUPS
    } else {
        DIRECT_LOOKUPS
    };
    format!(
        r#"var Alias = typeof Uint8Array === "function" ? Uint8Array : Array;
function template(input, destination) {{
{prefix}{initials}
destination = destination || new {allocator}(Math.floor(length * 3 / 4));
var a, b, c, d, e, f, g;
for (var inputCursor = 0, outputCursor = 0; inputCursor < length; inputCursor += 4, outputCursor += 3) {{
{lookups}    e = a << 2 | b >> 4;
    f = (b & 15) << 4 | c >> 2;
    g = (c & 3) << 6 | d;
    destination[outputCursor] = e;
    if (inputCursor + 2 < length) {{ destination[outputCursor + 1] = f; }}
    if (inputCursor + 3 < length) {{ destination[outputCursor + 2] = g; }}
  }}
  return destination;
}}"#,
        prefix = prefix,
        initials = initials,
        allocator = allocator,
        lookups = lookups,
    )
}

#[cfg(test)]
mod char_at_alias_tests {
    use super::*;

    #[test]
    fn retained_curried_char_at_alias_is_proven() {
        let source = include_str!("../../tests/fixtures/retained-decoder-alias.js");
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, source, SourceType::unambiguous()).parse();
        assert!(!parsed.panicked && parsed.errors.is_empty());
        let (scoping, nodes) = crate::analysis::analyze(&parsed.program);
        let before = source.find("function decode").expect("decoder function") as u32;
        let aliases = proven_char_at_aliases(&parsed.program, &scoping, &nodes, before);
        let names: BTreeSet<String> = aliases
            .iter()
            .map(|symbol| scoping.symbol_name(*symbol).to_string())
            .collect();
        assert!(names.contains("lookupChar"), "{names:?}");
        assert!(!names.contains("lookupIndex"), "{names:?}");
    }
}
