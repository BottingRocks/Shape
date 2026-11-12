//! CLEAN pass: collapse repeated base64/XOR string-decoder regions.
//!
//! Shape-style kernels decode string operands through the same boilerplate over
//! and over: read a string-table entry and a key index, build a `encoded + "," +
//! key` cache key, take a `typeof` fast path out of a null-prototype cache, then
//! base64-decode the key and the input, XOR the two byte buffers with a seed of
//! `(key[0] + input[0]) & 255`, and concatenate UTF-16 code units with
//! `String.fromCharCode`.
//!
//! This pass recognises that region structurally (Oxc AST + `SymbolId`
//! resolution), not by identifier spelling, and rewrites each complete region to
//! a single call to one shared helper. The helper is emitted at the end of the
//! nearest enclosing function body that provably owns the region's captured
//! decoder dependencies (its `SymbolId` declarations), so a kernel's handlers
//! share one hoisted declaration inside the kernel while an outer wrapper that
//! merely contains the kernel, a separate kernel, or an optional launcher gets
//! no claim on it. If the dependencies straddle scopes, the innermost enclosing
//! body (handler-local) is used rather than an unproved outer function. The
//! helper receives the original table, cache, decoder and `fromCharCode`
//! bindings explicitly, so shadowed lexical bindings remain valid without
//! generated free references. Near misses are left byte-for-byte unchanged and
//! reported. The XOR is never evaluated in Rust: unknown run-time inputs are
//! never turned into invented plaintext.

use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstKind};
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, ReferenceFlags, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::scope::ScopeFlags;

use crate::analysis::{self, Edit};

const DECODE_HELPER: &str = "vmDecodeString";

/// Outcome of the string-decoder cleanup.
#[derive(Debug, Default, Clone)]
pub struct DecodeOutcome {
    pub output: String,
    pub report: DecodeReport,
}

/// Counters and reasons recorded by [`run`].
#[derive(Debug, Default, Clone)]
pub struct DecodeReport {
    /// Decoder regions collapsed into a single `vmDecodeString` call.
    pub matched: usize,
    /// Retained for report compatibility. Whole-operation pairing is no longer
    /// used: each decoder region is rewritten independently.
    pub paired: usize,
    /// Total decoder-region rewrites applied.
    pub changed: usize,
    /// Chosen shared helper name.
    pub helper: Option<String>,
    /// Near misses and rejected candidates, with a reason.
    pub skipped: Vec<String>,
}

/// Run the CLEAN string-decoder pass over `source`.
pub fn run(source: &str) -> Result<DecodeOutcome, String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    // A `with` body or a direct `eval` makes identifier resolution dynamic: the
    // helper would capture the statically-resolved binding, not the run-time one.
    if program_has_dynamic_scope(&program, &scoping) {
        return Ok(DecodeOutcome {
            output: source.to_owned(),
            report: DecodeReport {
                skipped: vec![
                    "with or direct eval makes name resolution dynamic; string-decoder collapse declined"
                        .into(),
                ],
                ..Default::default()
            },
        });
    }
    let vm_shape = VmStackShape::detect(&nodes, &scoping);
    let mut collector = Collector {
        source,
        scoping: &scoping,
        nodes: &nodes,
        vm_shape,
        owners: Vec::new(),
        facts: Vec::new(),
        skipped: Vec::new(),
    };
    collector.visit_program(&program);
    let Collector {
        facts, mut skipped, ..
    } = collector;

    if facts.is_empty() {
        return Ok(DecodeOutcome {
            output: source.to_owned(),
            report: DecodeReport {
                skipped,
                ..Default::default()
            },
        });
    }

    let per_region: Vec<&RegionFact> = facts.iter().collect();
    let placements = plan_helper_placements(&facts, nodes.program().span.end, &scoping);
    if placements.is_empty() {
        return Ok(DecodeOutcome {
            output: source.to_owned(),
            report: DecodeReport {
                skipped,
                ..Default::default()
            },
        });
    }
    let helper = placements[0].helper.clone();
    let mut helpers = vec![String::new(); facts.len()];
    let mut edits = Vec::new();
    // Each helper is emitted in the body of the function that provably owns the
    // region's decoder dependencies. A function declaration is hoisted within
    // that body, so calls earlier in the same kernel resolve it, while an outer
    // wrapper, a launcher, or a separate kernel is not that dependency owner.
    // Only regions at program top level (no enclosing function) use a
    // program-scope helper.
    for placement in &placements {
        for &index in &placement.facts {
            helpers[index].clone_from(&placement.helper);
        }
        let text = decode_helper_text(&placement.helper);
        edits.push(Edit {
            start: placement.insert_offset,
            end: placement.insert_offset,
            replacement: format!("\n{text}\n"),
        });
    }
    edits.extend(build_region_edits(source, &per_region, &helpers, &scoping));

    let applied = analysis::apply_edits(source, edits);
    if !applied.rejected.is_empty() {
        // A partially applied region rewrite is not a proven rewrite.
        skipped.push(format!(
            "{}; source retained",
            analysis::describe_rejected_edits(&applied.rejected)
        ));
        return Ok(DecodeOutcome {
            output: source.to_owned(),
            report: DecodeReport {
                skipped,
                ..Default::default()
            },
        });
    }
    let next = applied.output;
    let allocator = Allocator::default();
    let reparsed = match analysis::parse_program(&allocator, &next) {
        Ok(program) => program,
        Err(_) => {
            skipped.push("rewrite did not reparse; source retained".into());
            return Ok(DecodeOutcome {
                output: source.to_owned(),
                report: DecodeReport {
                    skipped,
                    ..Default::default()
                },
            });
        }
    };
    let output = analysis::codegen(&reparsed);
    let matched = per_region.len();
    Ok(DecodeOutcome {
        output,
        report: DecodeReport {
            matched,
            paired: 0,
            changed: matched,
            helper: Some(helper),
            skipped,
        },
    })
}

/// Collapse each region to `var result = helper(encoded, key);` plus, for the
/// unlabeled shape, the original cache-hit statements (which now read the
/// helper result).
fn build_region_edits(
    source: &str,
    facts: &[&RegionFact],
    helpers: &[String],
    scoping: &Scoping,
) -> Vec<Edit> {
    facts
        .iter()
        .zip(helpers)
        .map(|(fact, helper)| {
            let encoded = slice(source, fact.encoded_var);
            let key = slice(source, fact.key);
            let mut replacement = String::new();
            if let Some(prefix) = fact.prefix {
                replacement.push_str(slice(source, prefix));
                replacement.push(' ');
            }
            let table = scoping.symbol_name(fact.bindings.table);
            let cache = scoping.symbol_name(fact.bindings.cache);
            let decoder = scoping.symbol_name(fact.bindings.decoder);
            let from_char_code = match fact.bindings.from_char_code {
                FromCharCode::Symbol(symbol) => scoping.symbol_name(symbol).to_owned(),
                FromCharCode::Intrinsic => "String.fromCharCode".to_owned(),
            };
            // Ordinary/labeled regions let the helper own the cache lookup and
            // store, so no `computeOnly` flag is passed. Only the conservative
            // nested-block form still asks for the pure computation and keeps its
            // explicit cache test and store.
            let full_cache = fact.labeled || fact.full_cache;
            let mode = if full_cache { "" } else { ", true" };
            replacement.push_str(&format!(
                "var {result} = {helper}({encoded}, {key}, {table}, {cache}, {decoder}, {from_char_code}{mode});",
                result = fact.result,
                helper = helper,
                table = table,
                cache = cache,
                decoder = decoder,
                from_char_code = from_char_code,
                mode = mode,
            ));
            // The hit statements already read the helper result (the cache-test
            // variable), so they are the faithful common tail once the helper owns
            // the cache. The conservative form instead keeps the miss tail and
            // redirects its explicit store at the pure helper result.
            let trailing = if full_cache { &fact.hit } else { &fact.miss };
            for span in trailing {
                replacement.push(' ');
                if !full_cache {
                    if let Some(value_span) = fact.miss_store_value {
                        if contains(*span, value_span) {
                            replacement.push_str(&replace_span(
                                source,
                                *span,
                                value_span,
                                &fact.result,
                            ));
                            continue;
                        }
                    }
                }
                replacement.push_str(slice(source, *span));
            }
            Edit {
                start: fact.edit_region.start,
                end: fact.edit_region.end,
                replacement,
            }
        })
        .collect()
}

fn decode_helper_text(name: &str) -> String {
    format!(
        "function {name}(encodedString, keyIndex, stringTable, cache, decodeBase64, fromCharCode, computeOnly) {{\n\
         \x20 if (!computeOnly) {{\n\
         \x20   var cacheKey = encodedString + \",\" + keyIndex;\n\
         \x20   var cachedString = cache[cacheKey];\n\
         \x20   if (typeof cachedString !== \"undefined\") {{\n\
         \x20     return cachedString;\n\
         \x20   }}\n\
         \x20 }}\n\
         \x20 var keyBytes = decodeBase64(stringTable[keyIndex]);\n\
         \x20 var encodedBytes = decodeBase64(encodedString);\n\
         \x20 var xorMask = keyBytes[0] + encodedBytes[0] & 255;\n\
         \x20 var decodedString = \"\";\n\
         \x20 for (var byteIndex = 1; byteIndex < keyBytes.length; ++byteIndex) {{\n\
         \x20   decodedString += fromCharCode(encodedBytes[byteIndex] ^ keyBytes[byteIndex] ^ xorMask);\n\
         \x20 }}\n\
         \x20 if (!computeOnly) {{\n\
         \x20   cache[cacheKey] = decodedString;\n\
         \x20 }}\n\
         \x20 return decodedString;\n\
         }}"
    )
}

fn replace_span(source: &str, outer: Span, target: Span, replacement: &str) -> String {
    let mut output = slice(source, outer).to_owned();
    let start = (target.start - outer.start) as usize;
    let end = (target.end - outer.start) as usize;
    output.replace_range(start..end, replacement);
    output
}

/// One shared decoder helper and the function body that owns it.
struct HelperPlacement {
    helper: String,
    /// Byte offset immediately before the owning body's closing brace, or the
    /// program end for a top-level (non-kernel) decoder.
    insert_offset: u32,
    /// Indices into the collected `facts` slice that call this helper.
    facts: Vec<usize>,
}

/// Group decoder regions by the function body that provably owns their
/// captured decoder dependencies.
///
/// The owner is derived from resolved declaration spans, never from function
/// nesting order or a name: the innermost enclosing body that declares the
/// region's table/cache/decoder/`fromCharCode` symbols is the kernel, so
/// operation functions nested inside it share one hoisted helper. An outer
/// wrapper that contains a kernel IIFE and a separate custom-event launcher is
/// not that owner, and two kernels inside one wrapper resolve to distinct
/// bodies with distinct helpers. When the dependencies straddle scopes, the
/// innermost enclosing body hosts a handler-local helper instead of the pass
/// guessing an outer wrapper. Regions with no enclosing function are a genuine
/// top-level/non-kernel fixture and keep the program-scope fallback.
fn plan_helper_placements(
    facts: &[RegionFact],
    program_end: u32,
    scoping: &Scoping,
) -> Vec<HelperPlacement> {
    let mut groups: std::collections::BTreeMap<Option<(u32, u32)>, Vec<usize>> =
        std::collections::BTreeMap::new();
    for (index, fact) in facts.iter().enumerate() {
        let owner = fact.helper_owner.map(|span| (span.start, span.end));
        groups.entry(owner).or_default().push(index);
    }

    let mut used = std::collections::BTreeSet::new();
    let mut placements = Vec::with_capacity(groups.len());
    for (owner, fact_indices) in groups {
        let helper = unique_name(DECODE_HELPER, scoping, &mut used);
        let insert_offset = match owner {
            // Function bodies span their braces, so insert just inside `}`.
            Some((_, end)) => end.saturating_sub(1),
            None => program_end,
        };
        placements.push(HelperPlacement {
            helper,
            insert_offset,
            facts: fact_indices,
        });
    }
    placements
}

/// Pick a name for a generated helper that no existing binding, unresolved
/// reference, or previously placed helper can capture.
fn unique_name(
    base: &str,
    scoping: &Scoping,
    used: &mut std::collections::BTreeSet<String>,
) -> String {
    let taken: std::collections::BTreeSet<&str> = scoping.symbol_names().collect();
    let unresolved: std::collections::BTreeSet<&str> = scoping
        .root_unresolved_references()
        .iter()
        .map(|(name, _)| name.as_str())
        .collect();
    let free = |candidate: &str| {
        !taken.contains(candidate) && !unresolved.contains(candidate) && !used.contains(candidate)
    };
    let chosen = if free(base) {
        base.to_owned()
    } else {
        (2..10_000)
            .map(|index| format!("{base}{index}"))
            .find(|candidate| free(candidate.as_str()))
            .unwrap_or_else(|| format!("{base}__"))
    };
    used.insert(chosen.clone());
    chosen
}

/// True when the program contains a `with` statement or an unresolved direct
/// `eval`, either of which makes identifier resolution dynamic.
fn program_has_dynamic_scope(program: &Program<'_>, scoping: &Scoping) -> bool {
    let mut visitor = DynamicScopeVisitor {
        scoping,
        dynamic: false,
    };
    visitor.visit_program(program);
    visitor.dynamic
}

struct DynamicScopeVisitor<'a> {
    scoping: &'a Scoping,
    dynamic: bool,
}

impl<'a> Visit<'a> for DynamicScopeVisitor<'a> {
    fn visit_with_statement(&mut self, _: &WithStatement<'a>) {
        self.dynamic = true;
    }

    fn visit_call_expression(&mut self, it: &CallExpression<'a>) {
        if let Expression::Identifier(identifier) = &it.callee {
            let unresolved = identifier.reference_id.get().is_none_or(|reference| {
                self.scoping.get_reference(reference).symbol_id().is_none()
            });
            if identifier.name == "eval" && unresolved {
                self.dynamic = true;
            }
        }
        walk::walk_call_expression(self, it);
    }
}

fn slice(source: &str, span: Span) -> &str {
    &source[span.start as usize..span.end as usize]
}

// ---------------------------------------------------------------------------
// Collection
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct BindingSet {
    table: SymbolId,
    cache: SymbolId,
    decoder: SymbolId,
    from_char_code: FromCharCode,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum FromCharCode {
    Symbol(SymbolId),
    Intrinsic,
}

#[derive(Clone)]
struct RegionFact {
    /// Span actually replaced when the region collapses on its own. For the
    /// unlabeled shape this excludes the cache-hit `if` so its early `return`
    /// stays intact; for the labeled shape it equals `region`.
    edit_region: Span,
    /// Statements to keep ahead of the replacement call (labeled variant).
    prefix: Option<Span>,
    /// Expression passed as the encoded-string argument.
    encoded_var: Span,
    /// Key-index expression.
    key: Span,
    labeled: bool,
    /// True when the helper owns the cache lookup and store and the hit
    /// statements are re-emitted as a common tail (ordinary regions). False for
    /// the conservative nested-block form that keeps the cache test in place.
    full_cache: bool,
    /// Result variable (labeled) or cache-test variable (unlabeled).
    result: String,
    /// Cache-hit statements, re-emitted after the call (unlabeled only).
    hit: Vec<Span>,
    /// Statements after the miss loop. The unlabeled rewrite retains these so
    /// stack LHS evaluation stays ahead of the explicit cache store.
    miss: Vec<Span>,
    /// The decoded text operand of the miss cache store, replaced by the pure
    /// helper result.
    miss_store_value: Option<Span>,
    bindings: BindingSet,
    /// Function body that provably owns the generated helper. This is the
    /// innermost enclosing body whose scope declares every captured decoder
    /// dependency, with an innermost (handler-local) fallback when those
    /// declarations straddle scopes. `None` only for a genuinely top-level
    /// region with no enclosing function.
    helper_owner: Option<Span>,
}

struct Collector<'a> {
    source: &'a str,
    scoping: &'a Scoping,
    nodes: &'a AstNodes<'a>,
    vm_shape: VmStackShape,
    /// Function bodies enclosing the current visit, outermost first.
    owners: Vec<Span>,
    facts: Vec<RegionFact>,
    skipped: Vec<String>,
}

impl<'a> Visit<'a> for Collector<'a> {
    fn visit_function(&mut self, it: &Function<'a>, flags: ScopeFlags) {
        let pushed = if let Some(body) = &it.body {
            self.owners.push(body.span);
            // A region whose miss tail ends the function is the only place the
            // cache-hit `return` can be folded: nothing runs after the region, so
            // the hit and miss paths coincide once the helper owns the cache.
            self.scan_unlabeled(&body.statements, true);
            true
        } else {
            false
        };
        walk::walk_function(self, it, flags);
        if pushed {
            self.owners.pop();
        }
    }

    fn visit_arrow_function_expression(&mut self, it: &ArrowFunctionExpression<'a>) {
        // An expression-bodied arrow has nowhere to host a helper declaration,
        // but a block body is a genuine function scope and must own its regions
        // just like a `function` body.
        let pushed = if it.expression {
            false
        } else {
            self.owners.push(it.body.span);
            self.scan_unlabeled(&it.body.statements, true);
            true
        };
        walk::walk_arrow_function_expression(self, it);
        if pushed {
            self.owners.pop();
        }
    }

    fn visit_labeled_statement(&mut self, it: &LabeledStatement<'a>) {
        if let Some(fact) = self.match_labeled(it) {
            self.facts.push(fact);
            return;
        }
        walk::walk_labeled_statement(self, it);
    }

    fn visit_block_statement(&mut self, it: &BlockStatement<'a>) {
        // Nested blocks keep the conservative cache mode: the hit `return` skips
        // statements after the block, so the cache test cannot be folded away.
        self.scan_unlabeled(&it.body, false);
        walk::walk_block_statement(self, it);
    }

    fn visit_for_statement(&mut self, it: &ForStatement<'a>) {
        // Record near misses so a decoder-looking loop that fails the arithmetic
        // or length proof is explicitly reported rather than silently kept.
        if looks_like_xor_loop(it) && match_for_loop(it, self.scoping).is_none() {
            self.skipped.push(format!(
                "xor-shaped loop at {}..{} does not match the proven arithmetic",
                it.span.start, it.span.end
            ));
        }
        walk::walk_for_statement(self, it);
    }

    fn visit_do_while_statement(&mut self, it: &DoWhileStatement<'a>) {
        if looks_like_xor_body(&it.body) {
            self.skipped.push(format!(
                "do-while XOR decoder at {}..{} declined: initial iteration is not proven equivalent",
                it.span.start, it.span.end
            ));
        }
        walk::walk_do_while_statement(self, it);
    }
}

impl<'a> Collector<'a> {
    /// Owners that actually contain the region. This filters out a function
    /// whose parameter list is being walked (body declarations are not visible
    /// to parameter initializers) and any other non-enclosing frame.
    fn enclosing_owners(&self, region: Span) -> Vec<Span> {
        self.owners
            .iter()
            .copied()
            .filter(|body| contains(*body, region))
            .collect()
    }

    /// The function body that provably owns the generated helper.
    ///
    /// The helper only needs to be visible to its call sites: every captured
    /// dependency is passed explicitly as an argument. Sharing one helper
    /// across the handlers of a kernel is therefore sound exactly when the
    /// kernel body is the innermost enclosing body that declares all of those
    /// dependencies. A generic outer wrapper that merely contains the kernel
    /// does not declare them, so it cannot claim the helper. If the dependency
    /// declarations straddle scopes (for example a kernel-level table plus a
    /// handler-local cache), the innermost enclosing body hosts a handler-local
    /// helper rather than the pass falling back to an unproved outer function.
    fn helper_owner(&self, region: Span, bindings: &BindingSet) -> Option<Span> {
        let owners = self.enclosing_owners(region);
        let declarations: Vec<Span> = dependency_symbols(bindings)
            .into_iter()
            .map(|symbol| {
                self.nodes
                    .get_node(self.scoping.symbol_declaration(symbol))
                    .span()
            })
            .collect();
        if let Some(owner) = owners.iter().rev().find(|owner| {
            declarations
                .iter()
                .all(|declaration| declaration_belongs_to(**owner, *declaration))
        }) {
            return Some(*owner);
        }
        owners.last().copied()
    }

    fn scan_unlabeled(&mut self, statements: &[Statement<'a>], function_body: bool) {
        for index in 0..statements.len() {
            if matches!(statements[index], Statement::IfStatement(_)) {
                if let Some(fact) = self.match_unlabeled(statements, index, function_body) {
                    self.facts.push(fact);
                }
            }
        }
    }

    fn match_unlabeled(
        &mut self,
        statements: &[Statement<'a>],
        if_index: usize,
        function_body: bool,
    ) -> Option<RegionFact> {
        if if_index < 3 {
            return None;
        }
        let encoded_stmt = &statements[if_index - 3];
        let cache_key_stmt = &statements[if_index - 2];
        let cache_read_stmt = &statements[if_index - 1];
        let if_stmt = match &statements[if_index] {
            Statement::IfStatement(inner) => inner,
            _ => return None,
        };

        let (encoded_binding, _) = single_var(encoded_stmt)?;
        let (cache_key_binding, cache_key_init) = single_var(cache_key_stmt)?;
        let (cached_binding, cache_read_init) = single_var(cache_read_stmt)?;

        let encoded_symbol = symbol_of_binding(encoded_binding)?;
        let cached = symbol_of_binding(cached_binding)?;
        if cache_test_symbol(if_stmt, self.scoping) != Some(cached) {
            return None;
        }
        let cache = cache_read_object(cache_read_init, self.scoping)?;
        if cache_is_effectful(self.scoping, self.nodes, cache) {
            self.skipped.push(format!(
                "unlabeled decoder region at {}..{} uses a Proxy or accessor-backed cache; cache action is effectful",
                cache_key_stmt.span().start,
                statements.last()?.span().end
            ));
            return None;
        }
        if symbol_of_expr(cache_read_index(cache_read_init)?, self.scoping)
            != Some(symbol_of_binding(cache_key_binding)?)
        {
            return None;
        }
        let (cache_key_encoded, key_symbol, key_span) =
            cache_key_parts(cache_key_init, self.scoping)?;
        if cache_key_encoded != encoded_symbol {
            return None;
        }

        let loop_index = (if_index + 1..statements.len())
            .find(|index| is_loop_statement(&statements[*index]))?;
        let decoded = match_decode_statements(statements, loop_index, self.scoping)?;
        if decoded.encoded_symbol != Some(encoded_symbol) || decoded.key_symbol != Some(key_symbol)
        {
            return None;
        }

        let (hit, miss) = unlabeled_hit_and_miss(statements, if_index, loop_index)?;
        let miss_store_value = cache_store_value_span(miss, cache, decoded.text, self.scoping)?;
        if !hit_and_miss_equivalent(
            self.source,
            self.scoping,
            &hit,
            miss,
            cached,
            decoded.text,
            cache,
        ) {
            self.skipped.push(format!(
                "unlabeled decoder region at {}..{} has cache-hit/cache-miss effects that differ ({} hit vs {} miss)",
                cache_key_stmt.span().start,
                statements.last()?.span().end,
                hit.len(),
                miss.len()
            ));
            return None;
        }

        let bindings = BindingSet {
            table: decoded.table,
            cache,
            decoder: decoded.decoder,
            from_char_code: decoded.fcc,
        };
        if let Some(symbol) = unstable_capture(self.scoping, &bindings) {
            self.skipped.push(format!(
                "unlabeled decoder region at {}..{} captures `{}`, which is reassigned; the helper would snapshot a stale value",
                cache_key_stmt.span().start,
                statements.last()?.span().end,
                self.scoping.symbol_name(symbol)
            ));
            return None;
        }
        let result = cached_binding.name.to_string();
        let last_end = statements.last()?.span().end;
        // Hoisting the cache store into the helper moves it ahead of the original
        // stack tail. That is faithful only when the cache write itself runs no
        // user code, the re-emitted tail cannot observe the cache, and the
        // encoded/key operands cannot be mutated by the helper's `ToPrimitive`
        // coercion. Otherwise keep the conservative compute-only call form, which
        // leaves the cache test and the explicit store exactly where they were.
        let hoist_proven = function_body
            && symbol_is_stable(self.scoping, encoded_symbol)
            && symbol_is_stable(self.scoping, key_symbol)
            && cache_plainness(self.scoping, self.nodes, cache) == CacheProof::Plain
            && tail_is_non_observable(self.scoping, self.nodes, &self.vm_shape, &hit);
        let (edit_region, full_cache) = if hoist_proven {
            (Span::new(cache_key_stmt.span().start, last_end), true)
        } else {
            (Span::new(if_stmt.span().end, last_end), false)
        };
        if function_body && !hoist_proven {
            self.skipped.push(format!(
                "unlabeled decoder region at {}..{}: cache-store hoist not proven non-observable ({}); kept the conservative compute-only cache form",
                cache_key_stmt.span().start,
                last_end,
                cache_hoist_blocker(
                    self.scoping,
                    self.nodes,
                    &self.vm_shape,
                    encoded_symbol,
                    key_symbol,
                    cache,
                    &hit
                ),
            ));
        }
        let helper_owner =
            self.helper_owner(Span::new(cache_key_stmt.span().start, last_end), &bindings);
        Some(RegionFact {
            edit_region,
            prefix: None,
            encoded_var: decoded.encoded_arg,
            key: key_span,
            labeled: false,
            full_cache,
            result,
            hit: hit.iter().map(|statement| statement.span()).collect(),
            miss: miss.iter().map(|statement| statement.span()).collect(),
            miss_store_value: Some(miss_store_value),
            bindings,
            helper_owner,
        })
    }

    fn match_labeled(&mut self, label: &LabeledStatement<'a>) -> Option<RegionFact> {
        let Statement::BlockStatement(block) = &label.body else {
            return None;
        };
        let statements = &block.body;
        let if_index = (0..statements.len()).find(|index| {
            matches!(&statements[*index], Statement::IfStatement(inner) if cache_test_symbol(inner, self.scoping).is_some())
        })?;
        if if_index < 2 {
            return None;
        }
        let cache_key_stmt = &statements[if_index - 2];
        let cache_read_stmt = &statements[if_index - 1];
        let if_stmt = match &statements[if_index] {
            Statement::IfStatement(inner) => inner,
            _ => return None,
        };
        let (cache_key_binding, cache_key_init) = single_var(cache_key_stmt)?;
        let (cached_binding, cache_read_init) = single_var(cache_read_stmt)?;
        let cached = symbol_of_binding(cached_binding)?;
        if cache_test_symbol(if_stmt, self.scoping) != Some(cached) {
            return None;
        }
        let cache = cache_read_object(cache_read_init, self.scoping)?;
        if cache_is_effectful(self.scoping, self.nodes, cache) {
            self.skipped.push(format!(
                "labeled decoder region at {}..{} uses a Proxy or accessor-backed cache; cache action is effectful",
                cache_key_stmt.span().start,
                statements.last()?.span().end
            ));
            return None;
        }
        if symbol_of_expr(cache_read_index(cache_read_init)?, self.scoping)
            != Some(symbol_of_binding(cache_key_binding)?)
        {
            return None;
        }
        let (cache_key_encoded, key_symbol, key_span) =
            cache_key_parts(cache_key_init, self.scoping)?;

        let loop_index = (if_index + 1..statements.len())
            .find(|index| is_loop_statement(&statements[*index]))?;
        let decoded = match_decode_statements(statements, loop_index, self.scoping)?;
        if decoded.key_symbol != Some(key_symbol) || cache_key_encoded != decoded.encoded_symbol? {
            return None;
        }
        let result = labeled_result(if_stmt)?;
        let store = statements.get(loop_index + 1)?;
        if !labeled_store_matches(store, result, cache, decoded.text, self.scoping) {
            return None;
        }
        let bindings = BindingSet {
            table: decoded.table,
            cache,
            decoder: decoded.decoder,
            from_char_code: decoded.fcc,
        };
        if let Some(symbol) = unstable_capture(self.scoping, &bindings) {
            self.skipped.push(format!(
                "labeled decoder region at {}..{} captures `{}`, which is reassigned; the helper would snapshot a stale value",
                cache_key_stmt.span().start,
                statements.last()?.span().end,
                self.scoping.symbol_name(symbol)
            ));
            return None;
        }
        // The helper also snapshots the encoded/key operands before the cache-key
        // `ToPrimitive` coercion; a reassigned operand would make the snapshot
        // diverge from the raw use point.
        if let Some(symbol) = [decoded.encoded_symbol?, key_symbol]
            .into_iter()
            .find(|&symbol| !symbol_is_stable(self.scoping, symbol))
        {
            self.skipped.push(format!(
                "labeled decoder region at {}..{} captures operand `{}`, which is reassigned; the helper would snapshot a stale value",
                cache_key_stmt.span().start,
                statements.last()?.span().end,
                self.scoping.symbol_name(symbol)
            ));
            return None;
        }
        let prefix = Span::new(
            statements.first()?.span().start,
            cache_key_stmt.span().start,
        );
        let helper_owner = self.helper_owner(label.span, &bindings);
        Some(RegionFact {
            edit_region: label.span,
            prefix: Some(prefix),
            encoded_var: decoded.encoded_arg,
            key: key_span,
            labeled: true,
            full_cache: true,
            result: result.name.to_string(),
            hit: Vec::new(),
            miss: Vec::new(),
            miss_store_value: None,
            bindings,
            helper_owner,
        })
    }
}

// ---------------------------------------------------------------------------
// Structural matchers
// ---------------------------------------------------------------------------

struct LoopShape {
    loop_var: SymbolId,
    text: SymbolId,
    decoded_input: SymbolId,
    decoded_key: SymbolId,
    xor_seed: SymbolId,
    fcc: FromCharCode,
    /// `while` / `do while` forms initialize the index in the statement just
    /// before the loop; `for` keeps the initializer in the loop node.
    external_init: bool,
}

struct Decoded {
    text: SymbolId,
    encoded_arg: Span,
    encoded_symbol: Option<SymbolId>,
    key_symbol: Option<SymbolId>,
    table: SymbolId,
    decoder: SymbolId,
    fcc: FromCharCode,
}

fn match_decode_statements<'a>(
    statements: &'a [Statement<'a>],
    loop_index: usize,
    scoping: &Scoping,
) -> Option<Decoded> {
    let shape = match_loop_statement(&statements[loop_index], scoping)?;
    let offset = usize::from(shape.external_init);
    if loop_index < 4 + offset {
        return None;
    }
    if shape.external_init {
        let (binding, init) = single_var(&statements[loop_index - 1])?;
        if symbol_of_binding(binding)? != shape.loop_var
            || !matches!(init, Expression::NumericLiteral(value) if value.value == 1.0)
        {
            return None;
        }
    }
    let index = |distance: usize| loop_index.checked_sub(distance + offset);

    let (text_binding, text_init) = single_var(&statements[index(1)?])?;
    if symbol_of_binding(text_binding)? != shape.text
        || !matches!(text_init, Expression::StringLiteral(value) if value.value.is_empty())
    {
        return None;
    }
    let (seed_binding, seed_init) = single_var(&statements[index(2)?])?;
    if symbol_of_binding(seed_binding)? != shape.xor_seed
        || !xor_seed_matches(seed_init, shape.decoded_key, shape.decoded_input, scoping)
    {
        return None;
    }
    let (input_binding, input_init) = single_var(&statements[index(3)?])?;
    if symbol_of_binding(input_binding)? != shape.decoded_input {
        return None;
    }
    let (encoded_arg, decoder) = decoder_call(input_init, scoping)?;
    let encoded_symbol = symbol_of_expr(encoded_arg, scoping);
    let (key_binding, key_init) = single_var(&statements[index(4)?])?;
    if symbol_of_binding(key_binding)? != shape.decoded_key {
        return None;
    }
    let (key_read_arg, key_decoder) = decoder_call(key_init, scoping)?;
    if key_decoder != decoder {
        return None;
    }
    // The key decoder usually consumes an intermediate `var key = table[index]`
    // declared immediately before it; follow that one alias so the table read is
    // resolved through Oxc symbols rather than through identifier spelling.
    let key_read = match symbol_of_expr(key_read_arg, scoping) {
        Some(symbol) => {
            let (read_binding, read_init) = single_var(&statements[index(5)?])?;
            if symbol_of_binding(read_binding)? != symbol {
                return None;
            }
            read_init
        }
        None => key_read_arg,
    };
    let key_symbol = match key_read {
        Expression::ComputedMemberExpression(member) => symbol_of_expr(&member.expression, scoping),
        _ => None,
    };
    let table = table_read(key_read, scoping)?;
    Some(Decoded {
        text: shape.text,
        encoded_arg: encoded_arg.span(),
        encoded_symbol,
        key_symbol,
        table,
        decoder,
        fcc: shape.fcc,
    })
}

fn match_loop_statement(statement: &Statement<'_>, scoping: &Scoping) -> Option<LoopShape> {
    match statement {
        Statement::ForStatement(for_stmt) => match_for_loop(for_stmt, scoping),
        Statement::WhileStatement(while_stmt) => {
            let (loop_var, decoded_key) = loop_test(&while_stmt.test, scoping)?;
            let assignment = loop_assignment(&while_stmt.body, loop_var, true, scoping)?;
            match_loop_assignment(assignment, loop_var, decoded_key, true, scoping)
        }
        // A bare do-while executes once even when the key buffer is empty or
        // has length one, unlike the canonical `for (i = 1; i < key.length)`.
        // Without a separate proof that the bound is greater than one, leave
        // it unchanged rather than claiming an equivalence we cannot prove.
        Statement::DoWhileStatement(_) => None,
        _ => None,
    }
}

fn match_for_loop(for_stmt: &ForStatement<'_>, scoping: &Scoping) -> Option<LoopShape> {
    let ForStatementInit::VariableDeclaration(init) = for_stmt.init.as_ref()? else {
        return None;
    };
    if init.declarations.len() != 1 {
        return None;
    }
    let declarator = &init.declarations[0];
    let loop_binding = simple_binding(&declarator.id)?;
    let loop_var = symbol_of_binding(loop_binding)?;
    let Some(Expression::NumericLiteral(one)) = declarator.init.as_ref() else {
        return None;
    };
    if one.value != 1.0 {
        return None;
    }

    let test = for_stmt.test.as_ref()?;
    let (test_var, decoded_key) = loop_test(test, scoping)?;
    if test_var != loop_var || !loop_increment(for_stmt.update.as_ref()?, loop_var, scoping) {
        return None;
    }
    let assignment = loop_assignment(&for_stmt.body, loop_var, false, scoping)?;
    match_loop_assignment(assignment, loop_var, decoded_key, false, scoping)
}

fn match_loop_assignment(
    assignment: &AssignmentExpression<'_>,
    loop_var: SymbolId,
    decoded_key: SymbolId,
    external_init: bool,
    scoping: &Scoping,
) -> Option<LoopShape> {
    if assignment.operator != AssignmentOperator::Addition {
        return None;
    }
    let text = assignment_target_symbol(&assignment.left, scoping)?;
    let Expression::CallExpression(call) = &assignment.right else {
        return None;
    };
    if call.arguments.len() != 1 {
        return None;
    }
    let fcc = from_char_code_callee(&call.callee, scoping)?;
    let argument = call.arguments[0].as_expression()?;
    let Expression::BinaryExpression(outer_xor) = argument else {
        return None;
    };
    if outer_xor.operator != BinaryOperator::BitwiseXOR {
        return None;
    }
    let xor_seed = symbol_of_expr(&outer_xor.right, scoping)?;
    let Expression::BinaryExpression(inner_xor) = &outer_xor.left else {
        return None;
    };
    if inner_xor.operator != BinaryOperator::BitwiseXOR {
        return None;
    }
    let decoded_input = member_index_symbol(&inner_xor.left, loop_var, scoping)?;
    let inner_key = member_index_symbol(&inner_xor.right, loop_var, scoping)?;
    if inner_key != decoded_key {
        return None;
    }
    Some(LoopShape {
        loop_var,
        text,
        decoded_input,
        decoded_key,
        xor_seed,
        fcc,
        external_init,
    })
}

/// Resolve the only direct intrinsic forms that native CLEAN is allowed to
/// expose. Identifier callees retain their lexical SymbolId; the global
/// `String.fromCharCode` and its `(0, ...)` indirect-call spelling are safe
/// under the same stable-builtin assumption used by native cleanup.
fn from_char_code_callee(callee: &Expression<'_>, scoping: &Scoping) -> Option<FromCharCode> {
    match callee {
        Expression::Identifier(identifier) => {
            Some(FromCharCode::Symbol(symbol_of_ref(scoping, identifier)?))
        }
        Expression::StaticMemberExpression(member)
            if member.property.name == "fromCharCode"
                && matches!(&member.object,
                    Expression::Identifier(identifier)
                        if identifier.name == "String" && symbol_of_ref(scoping, identifier).is_none()) =>
        {
            Some(FromCharCode::Intrinsic)
        }
        Expression::SequenceExpression(sequence)
            if sequence.expressions.len() == 2
                && matches!(&sequence.expressions[0], Expression::NumericLiteral(value) if value.value == 0.0) =>
        {
            from_char_code_callee(&sequence.expressions[1], scoping)
        }
        _ => None,
    }
}

fn loop_test(expression: &Expression<'_>, scoping: &Scoping) -> Option<(SymbolId, SymbolId)> {
    let Expression::BinaryExpression(comparison) = expression else {
        return None;
    };
    if comparison.operator != BinaryOperator::LessThan {
        return None;
    }
    let loop_var = symbol_of_expr(&comparison.left, scoping)?;
    let Expression::StaticMemberExpression(length) = &comparison.right else {
        return None;
    };
    if length.property.name != "length" {
        return None;
    }
    Some((loop_var, symbol_of_expr(&length.object, scoping)?))
}

fn loop_assignment<'a>(
    statement: &'a Statement<'a>,
    loop_var: SymbolId,
    with_body_increment: bool,
    scoping: &Scoping,
) -> Option<&'a AssignmentExpression<'a>> {
    if !with_body_increment {
        let body = single_body_expression(statement)?;
        let Expression::AssignmentExpression(assign) = body else {
            return None;
        };
        return Some(assign);
    }
    let Statement::BlockStatement(block) = statement else {
        return None;
    };
    if block.body.len() != 2 {
        return None;
    }
    let Statement::ExpressionStatement(expression) = &block.body[0] else {
        return None;
    };
    let Expression::AssignmentExpression(assign) = &expression.expression else {
        return None;
    };
    if !loop_increment_statement(&block.body[1], loop_var, scoping) {
        return None;
    }
    Some(assign)
}

fn loop_increment_statement(
    statement: &Statement<'_>,
    loop_var: SymbolId,
    scoping: &Scoping,
) -> bool {
    let Statement::ExpressionStatement(expression) = statement else {
        return false;
    };
    loop_increment(&expression.expression, loop_var, scoping)
}

fn loop_increment(expression: &Expression<'_>, loop_var: SymbolId, scoping: &Scoping) -> bool {
    match expression {
        Expression::UpdateExpression(update) => {
            update.operator == UpdateOperator::Increment
                && simple_target_symbol(&update.argument, scoping) == Some(loop_var)
        }
        Expression::AssignmentExpression(assign) => {
            let target = assignment_target_symbol(&assign.left, scoping);
            if target != Some(loop_var) {
                return false;
            }
            match assign.operator {
                AssignmentOperator::Addition => {
                    matches!(&assign.right, Expression::NumericLiteral(value) if value.value == 1.0)
                }
                AssignmentOperator::Assign => {
                    let Expression::BinaryExpression(add) = &assign.right else {
                        return false;
                    };
                    add.operator == BinaryOperator::Addition
                        && symbol_of_expr(&add.left, scoping) == Some(loop_var)
                        && matches!(&add.right, Expression::NumericLiteral(value) if value.value == 1.0)
                }
                _ => false,
            }
        }
        _ => false,
    }
}

fn is_loop_statement(statement: &Statement<'_>) -> bool {
    matches!(
        statement,
        Statement::ForStatement(_) | Statement::WhileStatement(_) | Statement::DoWhileStatement(_)
    )
}

/// A loose check used only to decide whether a non-matching loop deserves a
/// diagnostic.
fn looks_like_xor_loop(for_stmt: &ForStatement<'_>) -> bool {
    looks_like_xor_body(&for_stmt.body)
}

fn looks_like_xor_body(statement: &Statement<'_>) -> bool {
    let body = match statement {
        Statement::BlockStatement(block) => block.body.first(),
        _ => Some(statement),
    };
    let Some(Statement::ExpressionStatement(expression)) = body else {
        return false;
    };
    let Expression::AssignmentExpression(assign) = &expression.expression else {
        return false;
    };
    let Expression::CallExpression(call) = &assign.right else {
        return false;
    };
    matches!(&call.callee, Expression::Identifier(identifier) if identifier.name == "fromCharCode")
}

fn cache_test_symbol(if_stmt: &IfStatement<'_>, scoping: &Scoping) -> Option<SymbolId> {
    // The canonical fast path has no `else`; an alternate would add effects the
    // hit/miss equivalence proof does not model.
    if if_stmt.alternate.is_some() {
        return None;
    }
    let Expression::BinaryExpression(comparison) = &if_stmt.test else {
        return None;
    };
    if comparison.operator != BinaryOperator::StrictInequality {
        return None;
    }
    let Expression::UnaryExpression(unary) = &comparison.left else {
        return None;
    };
    if unary.operator != UnaryOperator::Typeof {
        return None;
    }
    let Expression::Identifier(identifier) = &unary.argument else {
        return None;
    };
    if !matches!(&comparison.right, Expression::StringLiteral(value) if value.value == "undefined")
    {
        return None;
    }
    symbol_of_ref(scoping, identifier)
}

fn cache_read_object<'a>(expression: &'a Expression<'a>, scoping: &Scoping) -> Option<SymbolId> {
    let Expression::ComputedMemberExpression(member) = expression else {
        return None;
    };
    symbol_of_expr(&member.object, scoping)
}

fn cache_read_index<'a>(expression: &'a Expression<'a>) -> Option<&'a Expression<'a>> {
    let Expression::ComputedMemberExpression(member) = expression else {
        return None;
    };
    Some(&member.expression)
}

/// The helper owns the cache lookup and store. That is only faithful when a
/// cache read/write cannot run observable user code. Positively-proved cases are
/// declined outright: a `Proxy`, an object literal or descriptor map with a
/// getter/setter, `Object.create(_, { accessor })`, or any later
/// `Object.defineProperty(cache, ...)`/`__defineGetter__` install. Unknown or
/// aliased origins are not declined here; they simply fail the later
/// plain-cache proof and fall back to the conservative compute-only form.
fn cache_is_effectful(scoping: &Scoping, nodes: &AstNodes<'_>, symbol: SymbolId) -> bool {
    if is_reflective_cache_target(scoping, nodes, symbol) {
        return true;
    }
    match declarator_init(scoping, nodes, symbol) {
        Some(Expression::NewExpression(new)) => {
            matches!(&new.callee, Expression::Identifier(proxy)
                if proxy.name == "Proxy" && symbol_of_ref(scoping, proxy).is_none())
        }
        Some(Expression::ObjectExpression(object)) => object_has_accessor(object),
        Some(Expression::CallExpression(call)) => object_create_is_effectful(call, scoping, nodes),
        _ => false,
    }
}

/// True when a later `Object.defineProperty(cache, ...)`, `Object.defineProperties`,
/// `Reflect.defineProperty`, or `cache.__defineGetter__/__defineSetter__` installs
/// an accessor on the cache binding, making later `cache[key]` reads effectful.
fn is_reflective_cache_target(scoping: &Scoping, nodes: &AstNodes<'_>, cache: SymbolId) -> bool {
    let mut probe = ReflectProbe {
        scoping,
        cache,
        found: false,
    };
    probe.visit_program(nodes.program());
    probe.found
}

struct ReflectProbe<'a> {
    scoping: &'a Scoping,
    cache: SymbolId,
    found: bool,
}

impl<'a> Visit<'a> for ReflectProbe<'a> {
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        self.inspect_call(call);
        walk::walk_call_expression(self, call);
    }
}

impl ReflectProbe<'_> {
    fn inspect_call(&mut self, call: &CallExpression<'_>) {
        let Expression::StaticMemberExpression(member) = &call.callee else {
            return;
        };
        if !matches!(
            member.property.name.as_str(),
            "defineProperty" | "defineProperties" | "__defineGetter__" | "__defineSetter__"
        ) {
            return;
        }
        let object_is_cache = symbol_of_expr(&member.object, self.scoping) == Some(self.cache);
        let first_is_cache = call
            .arguments
            .first()
            .and_then(Argument::as_expression)
            .and_then(|argument| symbol_of_expr(argument, self.scoping))
            == Some(self.cache);
        let global_form = matches!(&member.object, Expression::Identifier(identifier)
            if matches!(identifier.name.as_str(), "Object" | "Reflect")
                && symbol_of_ref(self.scoping, identifier).is_none());
        if object_is_cache || (global_form && first_is_cache) {
            self.found = true;
        }
    }
}

/// `Object.create(proto, descriptors)` installs accessors when the descriptor
/// map holds a `get`/`set` entry, or when it cannot be analyzed as all-data.
fn object_create_is_effectful(
    call: &CallExpression<'_>,
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
) -> bool {
    if !is_object_create(call, scoping, nodes) {
        return false;
    }
    match call.arguments.get(1) {
        None => false,
        Some(argument) => match argument.as_expression() {
            Some(Expression::ObjectExpression(object)) => !descriptor_map_is_all_data(object),
            Some(Expression::NullLiteral(_)) => true,
            _ => true,
        },
    }
}

fn is_object_create(call: &CallExpression<'_>, scoping: &Scoping, nodes: &AstNodes<'_>) -> bool {
    let resolved = resolve_alias(scoping, nodes, &call.callee);
    matches!(
        resolved,
        Expression::StaticMemberExpression(member)
            if member.property.name == "create"
                && is_global_object(scoping, nodes, &member.object)
    )
}

/// Follow a stable `var alias = <expr>` chain so an aliased native (for example
/// `Oa = OQ.create`, `OQ = Object`) still resolves to its origin. A reassigned
/// alias is not followed: its run-time value may differ from the initializer.
fn resolve_alias<'a>(
    scoping: &Scoping,
    nodes: &'a AstNodes<'a>,
    expression: &'a Expression<'a>,
) -> &'a Expression<'a> {
    let mut current = expression;
    for _ in 0..8 {
        let Expression::Identifier(identifier) = current else {
            break;
        };
        let Some(symbol) = symbol_of_ref(scoping, identifier) else {
            break;
        };
        if !symbol_is_stable(scoping, symbol) {
            break;
        }
        let Some(init) = declarator_init(scoping, nodes, symbol) else {
            break;
        };
        current = init;
    }
    current
}

/// `Object` reached either directly or through a stable alias chain.
fn is_global_object(scoping: &Scoping, nodes: &AstNodes<'_>, expression: &Expression<'_>) -> bool {
    matches!(resolve_alias(scoping, nodes, expression),
        Expression::Identifier(identifier)
            if identifier.name == "Object" && symbol_of_ref(scoping, identifier).is_none())
}

/// A descriptor map is all-data when every entry is a non-computed `Init`
/// property whose value is an object literal that never names `get`/`set`.
fn descriptor_map_is_all_data(object: &ObjectExpression<'_>) -> bool {
    object.properties.iter().all(|property| match property {
        ObjectPropertyKind::ObjectProperty(property) => {
            property.kind == PropertyKind::Init
                && !property.computed
                && matches!(&property.value, Expression::ObjectExpression(descriptor)
                    if !descriptor_names_accessor(descriptor))
        }
        ObjectPropertyKind::SpreadProperty(_) => false,
    })
}

fn descriptor_names_accessor(object: &ObjectExpression<'_>) -> bool {
    object.properties.iter().any(|property| match property {
        ObjectPropertyKind::ObjectProperty(property) => {
            property.kind != PropertyKind::Init
                || match &property.key {
                    PropertyKey::StaticIdentifier(identifier) => {
                        matches!(identifier.name.as_str(), "get" | "set")
                    }
                    PropertyKey::StringLiteral(string) => {
                        matches!(string.value.as_str(), "get" | "set")
                    }
                    _ => true,
                }
        }
        ObjectPropertyKind::SpreadProperty(_) => true,
    })
}

/// True when an object literal defines a getter or setter.
fn object_has_accessor(object: &ObjectExpression<'_>) -> bool {
    object.properties.iter().any(|property| {
        matches!(property, ObjectPropertyKind::ObjectProperty(property)
            if property.kind != PropertyKind::Init)
    })
}

/// Result of proving the cache object is a plain, local, non-escaping store.
#[derive(Clone, Copy, PartialEq, Eq)]
enum CacheProof {
    /// `Object.create(null)`, `{}`, or `[]` with no accessor/spread/reflective
    /// origin and no escaping use.
    Plain,
    /// Origin or uses are unknown; the cache write may run user code.
    Unknown,
}

fn cache_plainness(scoping: &Scoping, nodes: &AstNodes<'_>, symbol: SymbolId) -> CacheProof {
    let Some(init) = declarator_init(scoping, nodes, symbol) else {
        return CacheProof::Unknown;
    };
    let plain_init = match init {
        Expression::ObjectExpression(object) => object_literal_is_plain(object),
        Expression::ArrayExpression(array) => !array
            .elements
            .iter()
            .any(|element| matches!(element, ArrayExpressionElement::SpreadElement(_))),
        Expression::CallExpression(call) => object_create_is_plain(call, scoping, nodes),
        _ => false,
    };
    if !plain_init || !references_only_member_access(scoping, nodes, symbol) {
        return CacheProof::Unknown;
    }
    CacheProof::Plain
}

fn object_create_is_plain(
    call: &CallExpression<'_>,
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
) -> bool {
    if !is_object_create(call, scoping, nodes) {
        return false;
    }
    if !matches!(
        call.arguments.first().and_then(Argument::as_expression),
        Some(Expression::NullLiteral(_))
    ) {
        return false;
    }
    match call.arguments.get(1) {
        None => true,
        Some(argument) => matches!(argument.as_expression(),
            Some(Expression::ObjectExpression(object)) if descriptor_map_is_all_data(object)),
    }
}

/// A literal object with only non-computed data properties is a plain store.
fn object_literal_is_plain(object: &ObjectExpression<'_>) -> bool {
    object.properties.iter().all(|property| match property {
        ObjectPropertyKind::ObjectProperty(property) => {
            property.kind == PropertyKind::Init
                && !property.computed
                && !matches!(&property.key, PropertyKey::StaticIdentifier(identifier)
                    if identifier.name == "__proto__")
                && !matches!(&property.key, PropertyKey::StringLiteral(string)
                    if string.value == "__proto__")
        }
        ObjectPropertyKind::SpreadProperty(_) => false,
    })
}

fn references_only_member_access(
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    symbol: SymbolId,
) -> bool {
    scoping.get_resolved_references(symbol).all(|reference| {
        matches!(
            nodes.parent_kind(reference.node_id()),
            AstKind::ComputedMemberExpression(_) | AstKind::StaticMemberExpression(_)
        )
    })
}

fn symbol_is_stable(scoping: &Scoping, symbol: SymbolId) -> bool {
    !scoping
        .get_resolved_references(symbol)
        .any(|reference| reference.flags().contains(ReferenceFlags::Write))
}

fn cache_hoist_blocker(
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    vm_shape: &VmStackShape,
    encoded: SymbolId,
    key: SymbolId,
    cache: SymbolId,
    hit: &[&Statement<'_>],
) -> String {
    let mut reasons = Vec::new();
    if !symbol_is_stable(scoping, encoded) || !symbol_is_stable(scoping, key) {
        reasons.push("encoded/key operand is reassigned");
    }
    if cache_plainness(scoping, nodes, cache) != CacheProof::Plain {
        reasons.push("cache object is not a proven plain local store");
    }
    if !tail_is_non_observable(scoping, nodes, vm_shape, hit) {
        reasons.push("stack tail can observe the cache");
    }
    reasons.join("; ")
}

/// The re-emitted stack tail is non-observable when it performs no call-like or
/// reflective operation and every member access targets a proven-plain value,
/// including `context.<stackField>` for a proven private VM context.
fn tail_is_non_observable(
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    vm_shape: &VmStackShape,
    statements: &[&Statement<'_>],
) -> bool {
    let mut probe = ObservabilityProbe {
        scoping,
        nodes,
        vm_shape,
        observable: false,
        visited: Vec::new(),
    };
    for statement in statements {
        probe.visit_statement(statement);
    }
    !probe.observable
}

struct ObservabilityProbe<'a> {
    scoping: &'a Scoping,
    nodes: &'a AstNodes<'a>,
    vm_shape: &'a VmStackShape,
    observable: bool,
    visited: Vec<SymbolId>,
}

impl<'a> Visit<'a> for ObservabilityProbe<'a> {
    fn visit_call_expression(&mut self, _: &CallExpression<'a>) {
        self.observable = true;
    }

    fn visit_new_expression(&mut self, _: &NewExpression<'a>) {
        self.observable = true;
    }

    fn visit_tagged_template_expression(&mut self, _: &TaggedTemplateExpression<'a>) {
        self.observable = true;
    }

    fn visit_import_expression(&mut self, _: &ImportExpression<'a>) {
        self.observable = true;
    }

    fn visit_binary_expression(&mut self, expression: &BinaryExpression<'a>) {
        if matches!(
            expression.operator,
            BinaryOperator::In | BinaryOperator::Instanceof
        ) {
            self.observable = true;
        }
        walk::walk_binary_expression(self, expression);
    }

    fn visit_unary_expression(&mut self, expression: &UnaryExpression<'a>) {
        if expression.operator == UnaryOperator::Delete {
            self.observable = true;
        }
        walk::walk_unary_expression(self, expression);
    }

    fn visit_static_member_expression(&mut self, expression: &StaticMemberExpression<'a>) {
        if !self
            .vm_shape
            .member_is_plain_access(self.scoping, expression)
            && !self
                .vm_shape
                .is_stack_field_access(self.scoping, &expression.object)
            && !value_is_plain(
                self.scoping,
                self.nodes,
                &expression.object,
                &mut self.visited,
            )
        {
            self.observable = true;
        }
        walk::walk_static_member_expression(self, expression);
    }

    fn visit_computed_member_expression(&mut self, expression: &ComputedMemberExpression<'a>) {
        if !self
            .vm_shape
            .is_stack_field_access(self.scoping, &expression.object)
            && !value_is_plain(
                self.scoping,
                self.nodes,
                &expression.object,
                &mut self.visited,
            )
        {
            self.observable = true;
        }
        walk::walk_computed_member_expression(self, expression);
    }

    fn visit_object_expression(&mut self, expression: &ObjectExpression<'a>) {
        if !object_literal_is_plain(expression) {
            self.observable = true;
        }
        walk::walk_object_expression(self, expression);
    }

    fn visit_array_expression(&mut self, expression: &ArrayExpression<'a>) {
        if expression
            .elements
            .iter()
            .any(|element| matches!(element, ArrayExpressionElement::SpreadElement(_)))
        {
            self.observable = true;
        }
        walk::walk_array_expression(self, expression);
    }

    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        // An unresolved identifier is a global lookup, which may run a getter or
        // throw; both shift relative to the hoisted cache store.
        if symbol_of_ref(self.scoping, identifier).is_none() {
            self.observable = true;
        }
    }
}

fn value_is_plain(
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    expression: &Expression<'_>,
    visited: &mut Vec<SymbolId>,
) -> bool {
    match expression {
        Expression::ArrayExpression(array) => !array
            .elements
            .iter()
            .any(|element| matches!(element, ArrayExpressionElement::SpreadElement(_))),
        Expression::ObjectExpression(object) => object_literal_is_plain(object),
        Expression::Identifier(identifier) => symbol_of_ref(scoping, identifier)
            .is_some_and(|symbol| local_plain_binding(scoping, nodes, symbol, visited)),
        _ => false,
    }
}

fn local_plain_binding(
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    symbol: SymbolId,
    visited: &mut Vec<SymbolId>,
) -> bool {
    if visited.contains(&symbol) {
        return false;
    }
    visited.push(symbol);
    let plain = declarator_init(scoping, nodes, symbol).is_some_and(|init| {
        symbol_is_stable(scoping, symbol)
            && references_only_member_access(scoping, nodes, symbol)
            && value_is_plain(scoping, nodes, init, visited)
    });
    visited.pop();
    plain
}

// ---------------------------------------------------------------------------
// Private VM stack proof
// ---------------------------------------------------------------------------

/// A proven Shape-style private VM layout: a *stack factory* that returns a
/// plain array, a *context constructor* that assigns that factory's result to
/// two or more `this.<field>` slots, and the *operations callbacks* that receive
/// that context as their first parameter. On such a kernel a re-emitted
/// `context.<stackField>` tail cannot run user code, so the decoder helper may
/// own the cache store without reordering an observable effect.
///
/// The proof is tied to constructor and call identity rather than to pooled name
/// sets. `stack_fields` holds only the fields of the constructor whose instance
/// is actually fed to the callbacks, and `context_params` holds only the first
/// parameters of callbacks proven to receive that instance. A callback reaches
/// the VM context either through a computed-member call whose argument is a
/// proven `new <constructor>` instance (directly, or via the enclosing
/// function's own call site), or through the actual WebAssembly host-import
/// dataflow. Because the WASM boundary erases constructor identity, that path
/// additionally requires that the program have exactly one proven context
/// constructor. An unrelated holder or a generic dynamically-invoked helper must
/// never lend its fields or its parameters to the proof.
///
/// Synthetic kernels (a bare `O.TY` parameter with no proven context) and
/// local accessor/`Proxy` holders are deliberately unmatched, keeping them on
/// the conservative compute-only form.
#[derive(Default)]
struct VmStackShape {
    stack_fields: std::collections::BTreeSet<String>,
    context_params: std::collections::BTreeSet<SymbolId>,
}

impl VmStackShape {
    fn detect(nodes: &AstNodes<'_>, scoping: &Scoping) -> Self {
        let factories = stack_factories(nodes, scoping);
        if factories.is_empty() {
            return Self::default();
        }

        // Each constructor keeps its own stack fields. Field names are never
        // pooled across constructors, so an unrelated holder cannot lend its
        // field names to a real context.
        let mut constructors = std::collections::BTreeMap::new();
        for node in nodes.iter() {
            let AstKind::Function(function) = node.kind() else {
                continue;
            };
            let Some(symbol) = function.id.as_ref().and_then(|id| id.symbol_id.get()) else {
                continue;
            };
            if let Some(fields) = context_stack_fields(function, scoping, &factories) {
                constructors.insert(symbol, fields);
            }
        }
        // A constructor only counts once a `new` instance actually exists.
        constructors.retain(|symbol, _| constructed_with_new(nodes, scoping, *symbol));
        if constructors.is_empty() {
            return Self::default();
        }

        let mut context_params = std::collections::BTreeSet::new();
        let mut linked_fields = std::collections::BTreeSet::new();

        // Actual WebAssembly callback dataflow: a function handed to
        // `new WebAssembly.Instance` as a host import receives the privately
        // allocated VM context as its first argument. The boundary is opaque to
        // symbol dataflow, so the context constructor cannot be named from the
        // call site; under the static private-VM assumption exactly one proven
        // context constructor exists, otherwise the shape is declined.
        let wasm_params = wasm_import_context_params(nodes, scoping);
        if !wasm_params.is_empty() {
            context_params.extend(wasm_params);
            if constructors.len() == 1 {
                if let Some((_, fields)) = constructors.iter().next() {
                    linked_fields.extend(fields.iter().cloned());
                }
            }
        }

        // A function array invoked through a computed member with a proven
        // context argument is the operations table; only its callbacks' first
        // parameters are contexts, and only that argument's constructor fields
        // count as stack fields.
        for call in computed_member_context_calls(nodes, scoping) {
            let Some(ctor) = context_argument_ctor(&call, scoping, nodes, &constructors) else {
                continue;
            };
            if let Some(fields) = constructors.get(&ctor) {
                linked_fields.extend(fields.iter().cloned());
            }
            for function in function_array_functions(nodes, scoping, call.array) {
                if let Some(parameter) = first_param_symbol(function) {
                    context_params.insert(parameter);
                }
            }
        }

        VmStackShape {
            stack_fields: linked_fields,
            context_params,
        }
    }

    /// `context.<field>` where `context` is a proven operations-callback context
    /// parameter (the bare identifier) and `<field>` is a proven stack field.
    fn member_is_plain_access(
        &self,
        scoping: &Scoping,
        member: &StaticMemberExpression<'_>,
    ) -> bool {
        matches!(&member.object, Expression::Identifier(object)
            if symbol_of_ref(scoping, object)
                .is_some_and(|symbol| self.context_params.contains(&symbol)))
            && self.stack_fields.contains(member.property.name.as_str())
    }

    /// `context.<stackField>` used as an expression, e.g. the object of
    /// `context.stack.length` or `context.stack[index]`.
    fn is_stack_field_access(&self, scoping: &Scoping, expression: &Expression<'_>) -> bool {
        matches!(expression, Expression::StaticMemberExpression(member)
            if self.member_is_plain_access(scoping, member))
    }
}

/// Stack-field names assigned by `function`'s `this.<field> = <factory>()`
/// statements, or `None` when fewer than two are present. Two is the minimum
/// that distinguishes a context from an incidental holder.
fn context_stack_fields(
    function: &Function<'_>,
    scoping: &Scoping,
    factories: &std::collections::BTreeSet<SymbolId>,
) -> Option<std::collections::BTreeSet<String>> {
    let body = function.body.as_ref()?;
    let fields: std::collections::BTreeSet<String> = body
        .statements
        .iter()
        .filter_map(|statement| {
            let Statement::ExpressionStatement(expression) = statement else {
                return None;
            };
            let Expression::AssignmentExpression(assign) = &expression.expression else {
                return None;
            };
            if assign.operator != AssignmentOperator::Assign {
                return None;
            }
            let AssignmentTarget::StaticMemberExpression(member) = &assign.left else {
                return None;
            };
            if !matches!(member.object, Expression::ThisExpression(_)) {
                return None;
            }
            let Expression::CallExpression(call) = &assign.right else {
                return None;
            };
            let called = symbol_of_expr(&call.callee, scoping)?;
            if !factories.contains(&called) {
                return None;
            }
            Some(member.property.name.to_string())
        })
        .collect();
    (fields.len() >= 2).then_some(fields)
}

/// True when a `new <symbol>(...)` expression exists somewhere in the program.
fn constructed_with_new(nodes: &AstNodes<'_>, scoping: &Scoping, symbol: SymbolId) -> bool {
    nodes.iter().any(|node| {
        let AstKind::NewExpression(new) = node.kind() else {
            return false;
        };
        callee_symbol(&new.callee, scoping) == Some(symbol)
    })
}

fn callee_symbol(callee: &Expression<'_>, scoping: &Scoping) -> Option<SymbolId> {
    match callee {
        Expression::Identifier(identifier) => symbol_of_ref(scoping, identifier),
        _ => None,
    }
}

fn first_param_symbol(function: &Function<'_>) -> Option<SymbolId> {
    function
        .params
        .items
        .first()
        .and_then(|parameter| simple_binding(&parameter.pattern))
        .and_then(|binding| binding.symbol_id.get())
}

/// The function expressions of a `<symbol> = [function ...]` binding, or an
/// empty list when the binding is not a literal function array.
fn function_array_functions<'a>(
    nodes: &'a AstNodes<'a>,
    scoping: &Scoping,
    symbol: SymbolId,
) -> Vec<&'a Function<'a>> {
    let Some(Expression::ArrayExpression(array)) = declarator_init(scoping, nodes, symbol) else {
        return Vec::new();
    };
    array
        .elements
        .iter()
        .filter_map(|element| match element {
            ArrayExpressionElement::FunctionExpression(function) => Some(&**function),
            _ => None,
        })
        .collect()
}

/// A call `table[index](argument, ...)` where `table` is a literal function
/// array. The callback's first parameter is the operations context; the
/// argument identifies which constructor instance it receives.
struct ComputedContextCall<'a> {
    array: SymbolId,
    argument: &'a Expression<'a>,
    first_param: Option<SymbolId>,
    enclosing: Option<SymbolId>,
}

fn computed_member_context_calls<'a>(
    nodes: &'a AstNodes<'a>,
    scoping: &Scoping,
) -> Vec<ComputedContextCall<'a>> {
    let mut calls = Vec::new();
    for node in nodes.iter() {
        let AstKind::CallExpression(call) = node.kind() else {
            continue;
        };
        let Expression::ComputedMemberExpression(member) = &call.callee else {
            continue;
        };
        let Some(array) = symbol_of_expr(&member.object, scoping) else {
            continue;
        };
        let functions = function_array_functions(nodes, scoping, array);
        if functions.is_empty() {
            continue;
        }
        let Some(argument) = call.arguments.first().and_then(Argument::as_expression) else {
            continue;
        };
        let enclosing = nodes
            .ancestors(node.id())
            .find_map(|ancestor| match ancestor.kind() {
                AstKind::Function(function) => {
                    function.id.as_ref().and_then(|id| id.symbol_id.get())
                }
                _ => None,
            });
        let first_param = nodes
            .ancestors(node.id())
            .find_map(|ancestor| match ancestor.kind() {
                AstKind::Function(function) => first_param_symbol(function),
                _ => None,
            });
        calls.push(ComputedContextCall {
            array,
            argument,
            first_param,
            enclosing,
        });
    }
    calls
}

/// The constructor whose `new` instance reaches this computed call's context
/// argument, or `None` when identity cannot be proven.
fn context_argument_ctor(
    call: &ComputedContextCall<'_>,
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    constructors: &std::collections::BTreeMap<SymbolId, std::collections::BTreeSet<String>>,
) -> Option<SymbolId> {
    if let Some(ctor) = proven_context_expr(call.argument, scoping, nodes, constructors) {
        return Some(ctor);
    }
    // Otherwise the context may arrive as the enclosing function's own first
    // parameter, from a call that passes a proven `new <constructor>` instance.
    let enclosing = call.enclosing?;
    if call.first_param != symbol_of_expr(call.argument, scoping) {
        return None;
    }
    for node in nodes.iter() {
        let AstKind::CallExpression(candidate) = node.kind() else {
            continue;
        };
        if callee_symbol(&candidate.callee, scoping) != Some(enclosing) {
            continue;
        }
        let Some(first) = candidate
            .arguments
            .first()
            .and_then(Argument::as_expression)
        else {
            continue;
        };
        if let Some(ctor) = proven_context_expr(first, scoping, nodes, constructors) {
            return Some(ctor);
        }
    }
    None
}

/// The constructor of a proven context instance: a `new <constructor>` of a
/// proven context constructor, or a stable local bound to one.
fn proven_context_expr(
    expression: &Expression<'_>,
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    constructors: &std::collections::BTreeMap<SymbolId, std::collections::BTreeSet<String>>,
) -> Option<SymbolId> {
    proven_context_expr_at(expression, scoping, nodes, constructors, 0)
}

fn proven_context_expr_at(
    expression: &Expression<'_>,
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    constructors: &std::collections::BTreeMap<SymbolId, std::collections::BTreeSet<String>>,
    depth: u8,
) -> Option<SymbolId> {
    if depth > 8 {
        return None;
    }
    match expression {
        Expression::NewExpression(new) => {
            let ctor = callee_symbol(&new.callee, scoping)?;
            constructors.contains_key(&ctor).then_some(ctor)
        }
        Expression::Identifier(identifier) => {
            let symbol = symbol_of_ref(scoping, identifier)?;
            if !symbol_is_stable(scoping, symbol) {
                return None;
            }
            let init = declarator_init(scoping, nodes, symbol)?;
            proven_context_expr_at(init, scoping, nodes, constructors, depth + 1)
        }
        _ => None,
    }
}

/// First parameters of callbacks handed to `WebAssembly` as host imports. The
/// import object maps a module name to either an object of functions or a
/// function array; both forms receive the VM context as their first argument.
fn wasm_import_context_params(
    nodes: &AstNodes<'_>,
    scoping: &Scoping,
) -> std::collections::BTreeSet<SymbolId> {
    let mut params = std::collections::BTreeSet::new();
    for node in nodes.iter() {
        let AstKind::NewExpression(new) = node.kind() else {
            continue;
        };
        if !is_webassembly_instance(&new.callee, scoping, nodes) {
            continue;
        }
        let Some(Expression::ObjectExpression(imports)) =
            new.arguments.get(1).and_then(Argument::as_expression)
        else {
            continue;
        };
        for property in &imports.properties {
            let ObjectPropertyKind::ObjectProperty(property) = property else {
                continue;
            };
            match resolve_alias(scoping, nodes, &property.value) {
                Expression::ObjectExpression(module) => {
                    for entry in &module.properties {
                        let ObjectPropertyKind::ObjectProperty(entry) = entry else {
                            continue;
                        };
                        collect_context_callback(&entry.value, scoping, nodes, &mut params);
                    }
                }
                Expression::ArrayExpression(module) => {
                    collect_array_callback_params(module, &mut params);
                }
                _ => {}
            }
        }
    }
    params
}

fn collect_context_callback(
    expression: &Expression<'_>,
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    params: &mut std::collections::BTreeSet<SymbolId>,
) {
    match expression {
        Expression::FunctionExpression(function) => {
            if let Some(parameter) = first_param_symbol(function) {
                params.insert(parameter);
            }
        }
        Expression::Identifier(_) => match resolve_alias(scoping, nodes, expression) {
            Expression::FunctionExpression(function) => {
                if let Some(parameter) = first_param_symbol(function) {
                    params.insert(parameter);
                }
            }
            Expression::ArrayExpression(module) => {
                collect_array_callback_params(module, params);
            }
            _ => {}
        },
        _ => {}
    }
}

fn collect_array_callback_params(
    array: &ArrayExpression<'_>,
    params: &mut std::collections::BTreeSet<SymbolId>,
) {
    for element in &array.elements {
        if let ArrayExpressionElement::FunctionExpression(function) = element {
            if let Some(parameter) = first_param_symbol(function) {
                params.insert(parameter);
            }
        }
    }
}

fn is_webassembly_instance(
    callee: &Expression<'_>,
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
) -> bool {
    matches!(resolve_alias(scoping, nodes, callee),
        Expression::StaticMemberExpression(member)
            if member.property.name == "Instance"
                && matches!(resolve_alias(scoping, nodes, &member.object),
                    Expression::Identifier(global)
                        if global.name == "WebAssembly" && symbol_of_ref(scoping, global).is_none()))
}

/// Function symbols that build a plain VM stack: they return a local array
/// literal that is only ever extended through data-only `defineProperty` calls.
fn stack_factories(
    nodes: &AstNodes<'_>,
    scoping: &Scoping,
) -> std::collections::BTreeSet<SymbolId> {
    let mut factories = std::collections::BTreeSet::new();
    for node in nodes.iter() {
        let AstKind::Function(function) = node.kind() else {
            continue;
        };
        let Some(symbol) = function.id.as_ref().and_then(|id| id.symbol_id.get()) else {
            continue;
        };
        if is_stack_factory(function, scoping, nodes) {
            factories.insert(symbol);
        }
    }
    factories
}

fn is_stack_factory(function: &Function<'_>, scoping: &Scoping, nodes: &AstNodes<'_>) -> bool {
    let Some(body) = function.body.as_ref() else {
        return false;
    };
    let Some(returned) = body
        .statements
        .iter()
        .find_map(|statement| match statement {
            Statement::ReturnStatement(ret) => match &ret.argument {
                Some(Expression::Identifier(identifier)) => symbol_of_ref(scoping, identifier),
                _ => None,
            },
            _ => None,
        })
    else {
        return false;
    };
    if !symbol_is_stable(scoping, returned) {
        return false;
    }
    for statement in &body.statements {
        match statement {
            Statement::ReturnStatement(_) => {}
            Statement::VariableDeclaration(declaration) => {
                let only_array_local = declaration.declarations.len() == 1
                    && simple_binding(&declaration.declarations[0].id)
                        .and_then(|binding| binding.symbol_id.get())
                        == Some(returned)
                    && matches!(declaration.declarations[0].init.as_ref(),
                        Some(Expression::ArrayExpression(array))
                            if !array.elements.iter().any(|element| matches!(
                                element,
                                ArrayExpressionElement::SpreadElement(_))));
                if !only_array_local {
                    return false;
                }
            }
            Statement::ExpressionStatement(expression) => {
                let Expression::CallExpression(call) = &expression.expression else {
                    return false;
                };
                if !is_data_define_property_on(call, returned, scoping, nodes) {
                    return false;
                }
            }
            _ => return false,
        }
    }
    true
}

/// `<defineProperty>(local, key, { <data descriptor> })` for the factory's
/// returned array.
fn is_data_define_property_on(
    call: &CallExpression<'_>,
    local: SymbolId,
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
) -> bool {
    if call
        .arguments
        .first()
        .and_then(Argument::as_expression)
        .and_then(|argument| symbol_of_expr(argument, scoping))
        != Some(local)
    {
        return false;
    }
    let Some(Expression::ObjectExpression(descriptor)) =
        call.arguments.get(2).and_then(Argument::as_expression)
    else {
        return false;
    };
    if !descriptor_is_data(descriptor) {
        return false;
    }
    let resolved = resolve_alias(scoping, nodes, &call.callee);
    matches!(resolved,
        Expression::StaticMemberExpression(member)
            if member.property.name == "defineProperty"
                && is_global_object(scoping, nodes, &member.object))
}

/// A descriptor literal that installs a data property: every entry is a
/// non-computed `Init` and none is named `get`/`set`.
fn descriptor_is_data(descriptor: &ObjectExpression<'_>) -> bool {
    descriptor.properties.iter().all(|property| match property {
        ObjectPropertyKind::ObjectProperty(property) => {
            property.kind == PropertyKind::Init
                && !property.computed
                && descriptor_key_is_data(&property.key)
        }
        ObjectPropertyKind::SpreadProperty(_) => false,
    })
}

fn descriptor_key_is_data(key: &PropertyKey<'_>) -> bool {
    !matches!(key,
        PropertyKey::StaticIdentifier(identifier) if matches!(identifier.name.as_str(), "get" | "set"))
        && !matches!(key,
            PropertyKey::StringLiteral(string) if matches!(string.value.as_str(), "get" | "set"))
}

/// The generated helper receives each lexical dependency (table, cache, decoder,
/// `fromCharCode`) by value at call time and reuses that snapshot for the cache
/// lookup and both decoder calls. That is faithful only when the binding cannot
/// be reassigned; otherwise a later read would observe the updated value while
/// the helper kept the earlier one. Return the first captured binding that has a
/// write reference anywhere in the program.
fn unstable_capture(scoping: &Scoping, bindings: &BindingSet) -> Option<SymbolId> {
    if let FromCharCode::Symbol(symbol) = bindings.from_char_code {
        if !symbol_is_stable(scoping, symbol) {
            return Some(symbol);
        }
    }
    [bindings.table, bindings.cache, bindings.decoder]
        .into_iter()
        .find(|&symbol| !symbol_is_stable(scoping, symbol))
}

fn cache_store_value_span(
    statements: &[Statement<'_>],
    cache: SymbolId,
    text: SymbolId,
    scoping: &Scoping,
) -> Option<Span> {
    let mut finder = CacheStoreFinder {
        cache,
        text,
        scoping,
        value: None,
    };
    for statement in statements {
        finder.visit_statement(statement);
    }
    finder.value
}

struct CacheStoreFinder<'a> {
    cache: SymbolId,
    text: SymbolId,
    scoping: &'a Scoping,
    value: Option<Span>,
}

impl<'a, 'b> Visit<'b> for CacheStoreFinder<'a> {
    fn visit_assignment_expression(&mut self, assignment: &AssignmentExpression<'b>) {
        if assignment.operator == AssignmentOperator::Assign {
            if let AssignmentTarget::ComputedMemberExpression(member) = &assignment.left {
                if symbol_of_expr(&member.object, self.scoping) == Some(self.cache)
                    && symbol_of_expr(&assignment.right, self.scoping) == Some(self.text)
                {
                    if self.value.is_some() {
                        self.value = None;
                    } else {
                        self.value = Some(assignment.right.span());
                    }
                }
            }
        }
        walk::walk_assignment_expression(self, assignment);
    }
}

fn cache_key_parts<'a>(
    expression: &'a Expression<'a>,
    scoping: &Scoping,
) -> Option<(SymbolId, SymbolId, Span)> {
    let Expression::BinaryExpression(outer) = expression else {
        return None;
    };
    if outer.operator != BinaryOperator::Addition {
        return None;
    }
    let Expression::BinaryExpression(inner) = &outer.left else {
        return None;
    };
    if inner.operator != BinaryOperator::Addition {
        return None;
    }
    let Expression::StringLiteral(separator) = &inner.right else {
        return None;
    };
    if separator.value != "," {
        return None;
    }
    let encoded = symbol_of_expr(&inner.left, scoping)?;
    // The key is read once for the cache key and once for the table lookup. A
    // bare identifier is re-readable at no cost; any other expression may have
    // effects, so collapsing it into a single helper argument would change the
    // number (and value) of evaluations. Decline unless it is a plain binding.
    let key_symbol = symbol_of_expr(&outer.right, scoping)?;
    Some((encoded, key_symbol, outer.right.span()))
}

fn table_read<'a>(expression: &'a Expression<'a>, scoping: &Scoping) -> Option<SymbolId> {
    let Expression::ComputedMemberExpression(member) = expression else {
        return None;
    };
    symbol_of_expr(&member.object, scoping)
}

fn decoder_call<'a>(
    expression: &'a Expression<'a>,
    scoping: &Scoping,
) -> Option<(&'a Expression<'a>, SymbolId)> {
    let Expression::CallExpression(call) = expression else {
        return None;
    };
    if call.arguments.len() != 1 {
        return None;
    }
    let decoder = match &call.callee {
        Expression::Identifier(identifier) => symbol_of_ref(scoping, identifier)?,
        _ => return None,
    };
    Some((call.arguments[0].as_expression()?, decoder))
}

fn xor_seed_matches(
    expression: &Expression<'_>,
    decoded_key: SymbolId,
    decoded_input: SymbolId,
    scoping: &Scoping,
) -> bool {
    let Expression::BinaryExpression(and) = expression else {
        return false;
    };
    if and.operator != BinaryOperator::BitwiseAnd {
        return false;
    }
    if !matches!(&and.right, Expression::NumericLiteral(value) if value.value == 255.0) {
        return false;
    }
    let Expression::BinaryExpression(add) = &and.left else {
        return false;
    };
    if add.operator != BinaryOperator::Addition {
        return false;
    }
    member_index_key(&add.left, 0, scoping) == Some(decoded_key)
        && member_index_key(&add.right, 0, scoping) == Some(decoded_input)
}

fn unlabeled_hit_and_miss<'a>(
    statements: &'a [Statement<'a>],
    if_index: usize,
    for_index: usize,
) -> Option<(Vec<&'a Statement<'a>>, &'a [Statement<'a>])> {
    let Statement::IfStatement(if_stmt) = &statements[if_index] else {
        return None;
    };
    let Statement::BlockStatement(block) = &if_stmt.consequent else {
        return None;
    };
    let mut hit = Vec::new();
    for statement in &block.body {
        if let Statement::ReturnStatement(ret) = statement {
            if ret.argument.is_none() {
                continue;
            }
            return None;
        }
        hit.push(statement);
    }
    if hit.is_empty() {
        return None;
    }
    Some((hit, &statements[for_index + 1..]))
}

fn labeled_result<'a>(if_stmt: &'a IfStatement<'a>) -> Option<&'a BindingIdentifier<'a>> {
    let Statement::BlockStatement(block) = &if_stmt.consequent else {
        return None;
    };
    let (binding, _) = single_var(block.body.first()?)?;
    Some(binding)
}

fn labeled_store_matches(
    statement: &Statement<'_>,
    result: &BindingIdentifier<'_>,
    cache: SymbolId,
    text: SymbolId,
    scoping: &Scoping,
) -> bool {
    let Some((binding, init)) = single_var(statement) else {
        return false;
    };
    if binding.name != result.name {
        return false;
    }
    let Expression::AssignmentExpression(assign) = init else {
        return false;
    };
    if assign.operator != AssignmentOperator::Assign {
        return false;
    }
    let AssignmentTarget::ComputedMemberExpression(member) = &assign.left else {
        return false;
    };
    symbol_of_expr(&member.object, scoping) == Some(cache)
        && symbol_of_expr(&assign.right, scoping) == Some(text)
}

fn contains(outer: Span, inner: Span) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

/// Symbols whose declarations decide which function body owns the helper.
fn dependency_symbols(bindings: &BindingSet) -> Vec<SymbolId> {
    let mut symbols = vec![bindings.table, bindings.cache, bindings.decoder];
    if let FromCharCode::Symbol(symbol) = bindings.from_char_code {
        symbols.push(symbol);
    }
    symbols
}

/// True when `declaration` is declared in `owner`'s scope.
///
/// A `VariableDeclaration`/`Function` declaration is lexically inside the
/// owning body. A parameter or the function's own binding points at the
/// enclosing `Function` node, whose span contains that function's body, so the
/// function body also declares it.
fn declaration_belongs_to(owner: Span, declaration: Span) -> bool {
    contains(owner, declaration)
        || (declaration.start <= owner.start && owner.end <= declaration.end)
}

fn declarator_init<'a>(
    scoping: &Scoping,
    nodes: &'a AstNodes<'a>,
    symbol: SymbolId,
) -> Option<&'a Expression<'a>> {
    let declaration = scoping.symbol_declaration(symbol);
    if let AstKind::VariableDeclarator(declarator) = nodes.get_node(declaration).kind() {
        let binding = simple_binding(&declarator.id)?;
        if symbol_of_binding(binding) == Some(symbol) {
            return declarator.init.as_ref();
        }
        return None;
    }
    if let AstKind::VariableDeclarator(declarator) = nodes.parent_kind(declaration) {
        let binding = simple_binding(&declarator.id)?;
        if symbol_of_binding(binding) == Some(symbol) {
            return declarator.init.as_ref();
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Small AST helpers
// ---------------------------------------------------------------------------

fn simple_binding<'a>(pattern: &'a BindingPattern<'a>) -> Option<&'a BindingIdentifier<'a>> {
    match pattern {
        BindingPattern::BindingIdentifier(identifier) => Some(identifier),
        _ => None,
    }
}

fn symbol_of_binding(binding: &BindingIdentifier<'_>) -> Option<SymbolId> {
    binding.symbol_id.get()
}

fn symbol_of_ref(scoping: &Scoping, identifier: &IdentifierReference<'_>) -> Option<SymbolId> {
    crate::analysis::identifier_symbol(identifier, scoping)
}

fn symbol_of_expr(expression: &Expression<'_>, scoping: &Scoping) -> Option<SymbolId> {
    match expression {
        Expression::Identifier(identifier) => symbol_of_ref(scoping, identifier),
        _ => None,
    }
}

fn simple_target_symbol(
    target: &SimpleAssignmentTarget<'_>,
    scoping: &Scoping,
) -> Option<SymbolId> {
    match target {
        SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier) => {
            symbol_of_ref(scoping, identifier)
        }
        _ => None,
    }
}

fn assignment_target_symbol(target: &AssignmentTarget<'_>, scoping: &Scoping) -> Option<SymbolId> {
    match target {
        AssignmentTarget::AssignmentTargetIdentifier(identifier) => {
            symbol_of_ref(scoping, identifier)
        }
        _ => None,
    }
}

fn single_var<'a>(
    statement: &'a Statement<'a>,
) -> Option<(&'a BindingIdentifier<'a>, &'a Expression<'a>)> {
    let Statement::VariableDeclaration(declaration) = statement else {
        return None;
    };
    if declaration.declarations.len() != 1 {
        return None;
    }
    let declarator = &declaration.declarations[0];
    let binding = simple_binding(&declarator.id)?;
    Some((binding, declarator.init.as_ref()?))
}

fn single_body_expression<'a>(statement: &'a Statement<'a>) -> Option<&'a Expression<'a>> {
    match statement {
        Statement::ExpressionStatement(expression) => Some(&expression.expression),
        Statement::BlockStatement(block) if block.body.len() == 1 => match &block.body[0] {
            Statement::ExpressionStatement(expression) => Some(&expression.expression),
            _ => None,
        },
        _ => None,
    }
}

fn member_index_symbol(
    expression: &Expression<'_>,
    index: SymbolId,
    scoping: &Scoping,
) -> Option<SymbolId> {
    let Expression::ComputedMemberExpression(member) = expression else {
        return None;
    };
    if symbol_of_expr(&member.expression, scoping) != Some(index) {
        return None;
    }
    symbol_of_expr(&member.object, scoping)
}

fn member_index_key(
    expression: &Expression<'_>,
    index: u32,
    scoping: &Scoping,
) -> Option<SymbolId> {
    let Expression::ComputedMemberExpression(member) = expression else {
        return None;
    };
    match &member.expression {
        Expression::NumericLiteral(value) if value.value == f64::from(index) => {}
        _ => return None,
    }
    symbol_of_expr(&member.object, scoping)
}

// ---------------------------------------------------------------------------
// Effect-structure equivalence
// ---------------------------------------------------------------------------

fn hit_and_miss_equivalent(
    source: &str,
    scoping: &Scoping,
    hit: &[&Statement<'_>],
    miss: &[Statement<'_>],
    cached: SymbolId,
    text: SymbolId,
    cache: SymbolId,
) -> bool {
    if hit.len() != miss.len() {
        return false;
    }
    hit.iter()
        .zip(miss.iter())
        .all(|(hit_statement, miss_statement)| {
            normalize_statement(source, scoping, hit_statement, cached, text, cache)
                == normalize_statement(source, scoping, miss_statement, cached, text, cache)
        })
}

fn normalize_statement(
    source: &str,
    scoping: &Scoping,
    statement: &Statement<'_>,
    cached: SymbolId,
    text: SymbolId,
    cache: SymbolId,
) -> String {
    let mut visitor = Normalizer {
        scoping,
        cached,
        text,
        cache,
        identifiers: Vec::new(),
        stores: Vec::new(),
    };
    visitor.visit_statement(statement);
    let mut cuts: Vec<Span> = visitor.stores.clone();
    for identifier in &visitor.identifiers {
        if visitor
            .stores
            .iter()
            .any(|store| store.start <= identifier.start && identifier.end <= store.end)
        {
            continue;
        }
        cuts.push(*identifier);
    }
    cuts.sort_by_key(|span| span.start);
    let span = statement.span();
    let mut output = String::new();
    let mut cursor = span.start;
    for cut in cuts {
        if cut.start < cursor || cut.end > span.end {
            continue;
        }
        output.push_str(&source[cursor as usize..cut.start as usize]);
        output.push('_');
        cursor = cut.end;
    }
    output.push_str(&source[cursor as usize..span.end as usize]);
    output
        .chars()
        .filter(|character| !character.is_whitespace() && *character != ';')
        .collect()
}

struct Normalizer<'a> {
    scoping: &'a Scoping,
    cached: SymbolId,
    text: SymbolId,
    cache: SymbolId,
    identifiers: Vec<Span>,
    stores: Vec<Span>,
}

impl<'a, 'b> Visit<'b> for Normalizer<'a> {
    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'b>) {
        if let Some(symbol) = symbol_of_ref(self.scoping, identifier) {
            if symbol == self.cached || symbol == self.text {
                self.identifiers.push(identifier.span);
            }
        }
        walk::walk_identifier_reference(self, identifier);
    }

    fn visit_assignment_expression(&mut self, assignment: &AssignmentExpression<'b>) {
        if assignment.operator == AssignmentOperator::Assign {
            if let AssignmentTarget::ComputedMemberExpression(member) = &assignment.left {
                if symbol_of_expr(&member.object, self.scoping) == Some(self.cache)
                    && symbol_of_expr(&assignment.right, self.scoping) == Some(self.text)
                {
                    self.stores.push(assignment.span);
                }
            }
        }
        walk::walk_assignment_expression(self, assignment);
    }
}
