//! Independent, source-only cleanup of emitted `Object.defineProperty` writes.
//!
//! The symbolic emitter represents a VM array/object slot write as
//!
//! ```js
//! Object["defineProperty"](receiver, key, {
//!     writable: true,
//!     configurable: true,
//!     enumerable: true,
//!     value: value
//! });
//! ```
//!
//! This pass rewrites that exact shape to `receiver[key] = value` when, and
//! only when, the ordinary assignment is observably equivalent. It never
//! executes input JavaScript, never reads a captured capture script, and never
//! trusts a variable name or a minifier-style prefix. Every decision comes from
//! the Oxc AST and semantic symbol/reference graph.
//!
//! ## Why the descriptor must be exact
//!
//! `Object.defineProperty` and ordinary assignment differ unless the target
//! property is (or becomes) an ordinary own data property with
//! `writable`/`configurable`/`enumerable` all `true`:
//!
//! - an inherited setter runs on assignment but not on `defineProperty`;
//! - an own accessor setter runs on assignment and would be replaced by
//!   `defineProperty`;
//! - a non-writable or non-configurable own property makes the two operations
//!   fail differently (strict throw vs silent failure, or vice versa);
//! - `__proto__` on an ordinary object reaches the `Object.prototype` setter
//!   on assignment, while `defineProperty` creates an own data property;
//! - array `length` has different attributes and assignment semantics;
//! - a Proxy dispatches different traps;
//! - `defineProperty` returns the receiver while assignment returns the value.
//!
//! The pass therefore requires both:
//!
//! 1. the third argument is a plain object literal that owns exactly the four
//!    static data fields `writable: true`, `configurable: true`,
//!    `enumerable: true`, and `value: <expression>` (no spread, no accessors,
//!    no methods, no computed/dynamic flags, no duplicate or extra fields); and
//! 2. the receiver is a fresh array/object literal allocated as a direct
//!    statement of the same block, and the target property is either already a
//!    known own data property of that allocation or is pre-seeded (below)
//!    before any possible observation.
//!
//! `defineProperty` must be the native intrinsic of the unshadowed global
//! `Object`. The global authority check is fail-closed: the input is declined
//! when the bare global `Object` is written, escaped, aliased, or read other
//! than as the receiver of `Object.defineProperty`; when `globalThis`/`window`/
//! `self`/`global`/`top`/`parent`/`frames`/`this` can reach the global object;
//! when `eval`/`with` can run dynamic code; or when the `Function` constructor
//! is referenced, a constant `constructor` access is present, or an unknown
//! computed member on a function-like value is called. Calls through an unknown alias
//! (`var D = Object.defineProperty; D(...)`) are simply not rewritten.
//!
//! ## Safe pre-seeding
//!
//! A fresh `[]` or `{}` alone does **not** prove that a missing property can be
//! assigned: `Array.prototype` / `Object.prototype` may be patched, `__proto__`
//! is special for assignment, and array `length` reacts to writes. The pass
//! therefore creates the target property *inside the original literal*:
//!
//! ```js
//! let a = [];                          let a = [void 0];
//! Object.defineProperty(a, 0, {...});  a[0] = value;
//! ```
//!
//! For arrays, numeric keys up to [`MAX_PRESEED_ARRAY_ELEMENTS`] are emitted as
//! `void 0` elements, while untouched gaps remain elisions (holes), so the
//! final `length`, index layout, and hole set match the original. For objects,
//! missing string/numeric keys are appended as computed properties
//! (`{["name"]: void 0}`) in call order, preserving the original insertion
//! order for the whole literal.
//!
//! Pre-seeding is only used when every reference to the receiver before the
//! last rewritten write is the receiver of one of the same block's exact
//! `defineProperty` calls. Any earlier read, write, call argument, closure
//! capture, or other escape declines the whole symbol. References strictly
//! after the last write are allowed, because the object then has exactly the
//! shape the original calls produced. The receiver must not be reassigned, and
//! all candidate calls must be direct expression statements in the declaration
//! block (the call result is discarded), so evaluation count/order is
//! preserved: receiver once, key once, then value once.
//!
//! ## API and integration contract
//!
//! ```rust
//! use shape::emitted_cleanup::simplify_property_writes;
//!
//! let source = r#"function f() {
//!     let a = [];
//!     Object.defineProperty(a, 1, {
//!         writable: true,
//!         configurable: true,
//!         enumerable: true,
//!         value: 42,
//!     });
//!     return a.length;
//! }"#;
//! let outcome = simplify_property_writes(source).expect("valid JavaScript input");
//! assert_eq!(outcome.changed, 1);
//! assert!(outcome.code.contains("a[1] = 42;"));
//! assert!(outcome.code.contains("let a = [,void 0,];"));
//! assert!(outcome.skipped.is_empty());
//! ```
//!
//! [`simplify_property_writes`] returns [`PropertyWriteCleanupOutcome`] with:
//!
//! - `code`: the possibly rewritten source;
//! - `changed`: number of `defineProperty` calls rewritten;
//! - `preseeded_allocations` / `preseeded_keys`: literal edits and keys added;
//! - `skipped`: one [`PropertyWriteSkip`] per considered call that was left
//!   alone, with a typed reason and a human-readable detail string;
//! - `considered`: number of `Object.defineProperty`-shaped calls inspected;
//! - `status`, `notes`, and input/output byte lengths.
//!
//! For callers that already have `emit_javascript`/`cleanup_emitted_javascript`
//! artifacts, [`simplify_emitted_property_writes`] and
//! [`simplify_cleaned_emitted_javascript`] return
//! [`CleanedEmittedPropertyWrites`] with the same `statement_map`, `node_map`,
//! and `object_parameters`. Every edit is expression-local (a call expression
//! or an allocation literal) and adds/removes no statement, so statement
//! indexes and provenance are preserved rather than rebuilt. The CLI
//! integration point is to run this pass after `cleanup_emitted_javascript`
//! and before printing/returning the emitted artifact.
//!
//! ## Deliberate limitations
//!
//! - Only the direct `Object.defineProperty` / `Object["defineProperty"]`
//!   member callee is recognised; aliases and `.call` forms are retained.
//! - Program-level direct calls are retained because their result is the script
//!   completion value. Function and nested-block expression statements remain
//!   eligible when their result is discarded.
//! - The receiver must be a simple identifier bound to a fresh array/object
//!   literal. Parameters, call results, member reads, `Object.create(null)`,
//!   and arbitrary existing objects are retained.
//! - Calls inside branches, loops, nested functions, or non-expression
//!   statements are retained. The pass does not attempt cross-block dataflow.
//! - A parse/splice failure returns [`PropertyWriteCleanupError`]; the pass
//!   fails closed instead of returning a partially rewritten input.
//! - Re-generating a pre-seeded array literal drops comments that were inside
//!   the original literal's element list. Comments are not runtime behaviour,
//!   but this is a formatting change in the edited literal.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::{operator::BinaryOperator, scope::ScopeFlags};

use crate::analysis;
use crate::diagnostics::StageStatus;
use crate::js_number::number_to_js_string;
use crate::tracing::symbolic::{EmittedJavaScript, EmittedStatementMap, TraceNodeId};

use super::super::{CleanedEmittedJavaScript, StatementMapStatus};

mod authority;
mod calls;
mod contexts;
mod keys;
mod plan;

use authority::*;
use calls::*;
use contexts::*;
use keys::*;
use plan::*;

/// Maximum exclusive array index that a pre-seeded array literal may create.
/// Longer literals are retained rather than materialised huge.
pub const MAX_PRESEED_ARRAY_ELEMENTS: usize = 4096;

/// Maximum object-literal property count (existing plus pre-seeded).
pub const MAX_PRESEED_OBJECT_KEYS: usize = 256;

/// Parse, analysis, or output-safety failure. Input is never executed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropertyWriteCleanupError {
    Parse(String),
    InvalidOutput(String),
}

impl fmt::Display for PropertyWriteCleanupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => {
                write!(f, "property-write cleanup could not parse source: {error}")
            }
            Self::InvalidOutput(error) => {
                write!(f, "property-write cleanup produced invalid output: {error}")
            }
        }
    }
}

impl std::error::Error for PropertyWriteCleanupError {}

/// Why one `Object.defineProperty`-shaped call was retained.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PropertyWriteSkipReason {
    /// `with` or a reference to `eval` makes the semantic graph incomplete.
    DynamicScope,
    /// The input can reach the global object (`this`, `globalThis`, ...).
    GlobalObjectEscape,
    /// The `Function` constructor can run dynamic code.
    DynamicCode,
    /// The bare global `Object` binding was written, escaped, or aliased.
    NativeObjectEscaped,
    /// `Object` at the call site resolves to a local binding.
    ShadowedObject,
    /// The call is part of an optional chain.
    OptionalChain,
    /// The call does not have exactly three non-spread arguments.
    WrongArgumentCount,
    /// The receiver argument is not a plain identifier.
    ReceiverNotIdentifier,
    /// The receiver identifier does not resolve to a local binding.
    ReceiverUnknown,
    /// The receiver binding has no provable fresh array/object initializer.
    ReceiverNotFreshAllocation,
    /// The receiver binding is reassigned somewhere.
    ReceiverReassigned,
    /// The receiver has a formal-parameter role (sloppy arguments aliasing).
    ParameterAliasing,
    /// The binding is declared more than once with an initializer.
    DuplicateDeclaration,
    /// The receiver is read, mutated, or captured before the last write.
    ReceiverObservedBeforeWrites,
    /// The receiver is referenced from a nested closure scope.
    ReceiverEscaped,
    /// The call is not a direct expression statement in the declaration block.
    NotDirectExpressionStatement,
    /// The call is in a different block from the receiver allocation.
    CrossBlockWrite,
    /// The allocation literal contains a spread or dynamic/accessor shape.
    LiteralNotEligible,
    /// The key argument is not a side-effect-free static property key.
    NonLiteralKey,
    /// The key is `__proto__`, which is special for ordinary assignment.
    ProtoKey,
    /// The descriptor is not exactly the plain four-field data descriptor.
    DescriptorNotExact,
    /// The key is absent and cannot be created inside the literal.
    KeyNotPreseedable,
    /// Pre-seeding would exceed the configured literal limits.
    PreseedLimit,
    /// The generated-program direct-write proof did not cover this call.
    ProgramRewriteUnproven,
}

impl PropertyWriteSkipReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DynamicScope => "dynamic_scope",
            Self::GlobalObjectEscape => "global_object_escape",
            Self::DynamicCode => "dynamic_code",
            Self::NativeObjectEscaped => "native_object_escaped",
            Self::ShadowedObject => "shadowed_object",
            Self::OptionalChain => "optional_chain",
            Self::WrongArgumentCount => "wrong_argument_count",
            Self::ReceiverNotIdentifier => "receiver_not_identifier",
            Self::ReceiverUnknown => "receiver_unknown",
            Self::ReceiverNotFreshAllocation => "receiver_not_fresh_allocation",
            Self::ReceiverReassigned => "receiver_reassigned",
            Self::ParameterAliasing => "parameter_aliasing",
            Self::DuplicateDeclaration => "duplicate_declaration",
            Self::ReceiverObservedBeforeWrites => "receiver_observed_before_writes",
            Self::ReceiverEscaped => "receiver_escaped",
            Self::NotDirectExpressionStatement => "not_direct_expression_statement",
            Self::CrossBlockWrite => "cross_block_write",
            Self::LiteralNotEligible => "literal_not_eligible",
            Self::NonLiteralKey => "non_literal_key",
            Self::ProtoKey => "proto_key",
            Self::DescriptorNotExact => "descriptor_not_exact",
            Self::KeyNotPreseedable => "key_not_preseedable",
            Self::PreseedLimit => "preseed_limit",
            Self::ProgramRewriteUnproven => "program_rewrite_unproven",
        }
    }
}

impl fmt::Display for PropertyWriteSkipReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One retained call and the exact reason it was not rewritten.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertyWriteSkip {
    /// Original input byte range of the call expression.
    pub span: (u32, u32),
    pub reason: PropertyWriteSkipReason,
    pub detail: String,
}

/// Result of one [`simplify_property_writes`] run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertyWriteCleanupOutcome {
    pub code: String,
    /// Number of `defineProperty` calls rewritten to assignments.
    pub changed: usize,
    /// Number of fresh allocation literals that gained pre-seeded keys.
    pub preseeded_allocations: usize,
    /// Number of keys added to allocation literals.
    pub preseeded_keys: usize,
    /// Number of `Object.defineProperty`-shaped calls inspected.
    pub considered: usize,
    /// Every inspected call that is still present, in source order.
    pub skipped: Vec<PropertyWriteSkip>,
    pub status: StageStatus,
    pub notes: Vec<String>,
    pub input_bytes: usize,
    pub output_bytes: usize,
}

/// Rewritten source plus the preserved provenance maps of an emitted artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanedEmittedPropertyWrites {
    pub code: String,
    pub statement_map: Vec<EmittedStatementMap>,
    pub node_map: BTreeMap<TraceNodeId, usize>,
    pub object_parameters: BTreeMap<u64, String>,
    /// Expression-local edits add/remove no statements, so the caller's maps
    /// remain exact and this is always [`StatementMapStatus::Preserved`].
    pub statement_map_status: StatementMapStatus,
    pub outcome: PropertyWriteCleanupOutcome,
}

/// Rewrite provably equivalent `Object.defineProperty` data writes.
pub fn simplify_property_writes(
    source: &str,
) -> Result<PropertyWriteCleanupOutcome, PropertyWriteCleanupError> {
    let allocator = Allocator::default();
    let program =
        analysis::parse_program(&allocator, source).map_err(PropertyWriteCleanupError::Parse)?;
    let (scoping, nodes) = analysis::analyze(&program);

    let mut contexts = ContextCollector::default();
    contexts.visit_program(&program);

    let mut scan = ScanCollector {
        scoping: &scoping,
        raw_calls: Vec::new(),
        define_property_members: Vec::new(),
        write_spans: Vec::new(),
        saw_with: false,
        saw_eval_reference: false,
        saw_this: false,
        saw_constructor_access: false,
        saw_unknown_computed_call: false,
    };
    scan.visit_program(&program);

    let calls = build_calls(&scan.raw_calls, &contexts, &scoping);
    let considered = calls.len();

    let authority = NativeAuthority::evaluate(&scoping, &nodes, &scan);
    if let Some((reason, detail)) = authority.declined {
        return Ok(declined_outcome(source, &calls, considered, reason, detail));
    }

    let mut writes = ReceiverWrites::new(&calls);
    let groups = writes.group_by_receiver(&calls);
    let receivers = ReceiverContext {
        source,
        scoping: &scoping,
        nodes: &nodes,
        contexts: &contexts,
        calls: &calls,
    };
    for (symbol, indices) in groups {
        writes.plan_receiver(&receivers, symbol, indices);
    }

    writes.apply(source, &calls, considered)
}

/// The unchanged outcome when the native `Object.defineProperty` authority
/// check declines the whole input: every call is skipped with its reason.
fn declined_outcome(
    source: &str,
    calls: &[CandidateCall],
    considered: usize,
    reason: PropertyWriteSkipReason,
    detail: String,
) -> PropertyWriteCleanupOutcome {
    let skipped = calls
        .iter()
        .map(|call| {
            call.skip.clone().unwrap_or_else(|| PropertyWriteSkip {
                span: span_pair(call.span),
                reason,
                detail: detail.clone(),
            })
        })
        .collect::<Vec<_>>();
    PropertyWriteCleanupOutcome {
        code: source.to_owned(),
        changed: 0,
        preseeded_allocations: 0,
        preseeded_keys: 0,
        considered,
        skipped,
        status: StageStatus::Unsupported,
        notes: vec![detail],
        input_bytes: source.len(),
        output_bytes: source.len(),
    }
}

/// What the per-receiver planning reads: the source, its semantic graph, the
/// collected block contexts, and every candidate call.
struct ReceiverContext<'r, 'a> {
    source: &'r str,
    scoping: &'r Scoping,
    nodes: &'r AstNodes<'a>,
    contexts: &'r ContextCollector,
    calls: &'r [CandidateCall],
}

/// Per-call skips and rewrites plus the pre-seed edits, accumulated over
/// every receiver group.
struct ReceiverWrites {
    skip_by_call: Vec<Option<PropertyWriteSkip>>,
    rewrite_by_call: Vec<Option<String>>,
    preseed_edits: Vec<analysis::Edit>,
    preseeded_allocations: usize,
    preseeded_keys: usize,
}

impl ReceiverWrites {
    fn new(calls: &[CandidateCall]) -> Self {
        Self {
            skip_by_call: calls.iter().map(|call| call.skip.clone()).collect(),
            rewrite_by_call: vec![None; calls.len()],
            preseed_edits: Vec::new(),
            preseeded_allocations: 0,
            preseeded_keys: 0,
        }
    }

    /// Group the unskipped calls by receiver symbol; a receiver that is not a
    /// local binding skips its call.
    fn group_by_receiver(&mut self, calls: &[CandidateCall]) -> BTreeMap<SymbolId, Vec<usize>> {
        let mut groups: BTreeMap<SymbolId, Vec<usize>> = BTreeMap::new();
        for (index, call) in calls.iter().enumerate() {
            if call.skip.is_some() {
                continue;
            }
            let Some(symbol) = call.receiver_symbol else {
                self.skip_by_call[index] = Some(PropertyWriteSkip {
                    span: span_pair(call.span),
                    reason: PropertyWriteSkipReason::ReceiverUnknown,
                    detail: "receiver does not resolve to a local binding".into(),
                });
                continue;
            };
            groups.entry(symbol).or_default().push(index);
        }
        groups
    }

    /// Prove one receiver's calls are rewritable and record their rewrites
    /// and pre-seed edit, or skip each with the reason it failed.
    fn plan_receiver(
        &mut self,
        receivers: &ReceiverContext<'_, '_>,
        symbol: SymbolId,
        mut indices: Vec<usize>,
    ) {
        let ReceiverContext {
            source,
            scoping,
            nodes,
            contexts,
            calls,
        } = *receivers;
        let skip_by_call = &mut self.skip_by_call;
        let Some(plan) = contexts.declarations.get(&symbol) else {
            for index in indices {
                set_skip(
                    skip_by_call,
                    calls,
                    index,
                    PropertyWriteSkipReason::ReceiverNotFreshAllocation,
                    "receiver has no fresh array/object literal declaration",
                );
            }
            return;
        };
        if contexts.duplicate_symbols.contains(&symbol) {
            for index in indices {
                set_skip(
                    skip_by_call,
                    calls,
                    index,
                    PropertyWriteSkipReason::DuplicateDeclaration,
                    "receiver binding has more than one initializer",
                );
            }
            return;
        }
        if contexts.parameter_symbols.contains(&symbol) {
            for index in indices {
                set_skip(
                    skip_by_call,
                    calls,
                    index,
                    PropertyWriteSkipReason::ParameterAliasing,
                    "receiver has a formal-parameter role; sloppy `arguments` aliasing can observe the fresh allocation",
                );
            }
            return;
        }

        // Every candidate used by a pre-seed must be a direct statement after
        // the declaration and in the same block as the allocation literal.
        let declaration_block = plan.block_id;
        let mut same_block = Vec::new();
        for index in indices.drain(..) {
            let call = &calls[index];
            let Some(context) = call.context else {
                set_skip(
                    skip_by_call,
                    calls,
                    index,
                    PropertyWriteSkipReason::NotDirectExpressionStatement,
                    "call is not a direct expression statement",
                );
                continue;
            };
            if context.block_id != declaration_block
                || context.statement_index <= plan.statement_index
            {
                set_skip(
                    skip_by_call,
                    calls,
                    index,
                    PropertyWriteSkipReason::CrossBlockWrite,
                    "write is not a direct statement after the allocation in the same block",
                );
                continue;
            }
            same_block.push(index);
        }
        if same_block.is_empty() {
            return;
        }
        same_block.sort_by_key(|index| calls[*index].context.unwrap().statement_index);
        let last_write = calls[*same_block.last().unwrap()]
            .context
            .unwrap()
            .statement_index;

        let safe_receivers = same_block
            .iter()
            .filter_map(|index| calls[*index].receiver_ident_span.map(|span| span.start))
            .collect::<BTreeSet<_>>();

        if let Err((reason, detail)) = receiver_references_are_safe(
            scoping,
            nodes,
            symbol,
            last_write,
            &contexts.blocks[plan.block_id].statement_spans,
            &safe_receivers,
        ) {
            for index in same_block {
                set_skip(skip_by_call, calls, index, reason, &detail);
            }
            return;
        }

        let group_plan = plan_symbol_writes(source, calls, &same_block, plan);
        for (index, reason, detail) in group_plan.skips {
            set_skip(skip_by_call, calls, index, reason, &detail);
        }
        for (index, replacement) in group_plan.rewrites {
            self.rewrite_by_call[index] = Some(replacement);
        }
        if let Some(edit) = group_plan.preseed_edit {
            self.preseed_edits.push(edit);
            self.preseeded_allocations += 1;
            self.preseeded_keys += group_plan.preseeded_keys;
        }
    }

    /// Splice the pre-seeds and rewrites into `source`, re-parse the result,
    /// and report the outcome.
    fn apply(
        self,
        source: &str,
        calls: &[CandidateCall],
        considered: usize,
    ) -> Result<PropertyWriteCleanupOutcome, PropertyWriteCleanupError> {
        let ReceiverWrites {
            skip_by_call,
            rewrite_by_call,
            preseed_edits,
            preseeded_allocations,
            preseeded_keys,
        } = self;
        let mut edits = preseed_edits;
        changed_rewrites(&mut edits, calls, &rewrite_by_call);

        let mut skipped = skip_by_call.into_iter().flatten().collect::<Vec<_>>();
        skipped.sort_by_key(|skip| skip.span.0);

        let applied = analysis::apply_edits(source, edits);
        if !applied.rejected.is_empty() {
            return Err(PropertyWriteCleanupError::InvalidOutput(
                analysis::describe_rejected_edits(&applied.rejected),
            ));
        }
        if applied.applied == 0 {
            return Ok(PropertyWriteCleanupOutcome {
                code: source.to_owned(),
                changed: 0,
                preseeded_allocations: 0,
                preseeded_keys: 0,
                considered,
                skipped,
                status: StageStatus::Unknown,
                notes: if considered == 0 {
                    vec!["no Object.defineProperty-shaped calls were present".into()]
                } else {
                    vec!["no call met the exact rewrite proof; input returned unchanged".into()]
                },
                input_bytes: source.len(),
                output_bytes: source.len(),
            });
        }

        // Re-parse before returning. The pass never executes JavaScript, and a
        // malformed splice must fail closed.
        let output_allocator = Allocator::default();
        analysis::parse_program(&output_allocator, &applied.output)
            .map_err(PropertyWriteCleanupError::InvalidOutput)?;

        let changed = rewrite_by_call.iter().flatten().count();
        Ok(PropertyWriteCleanupOutcome {
            output_bytes: applied.output.len(),
            code: applied.output,
            changed,
            preseeded_allocations,
            preseeded_keys,
            considered,
            skipped,
            status: if changed > 0 {
                StageStatus::Found
            } else {
                StageStatus::Unknown
            },
            notes: vec![
                format!(
                    "rewrote {changed} defineProperty call(s); pre-seeded {preseeded_keys} key(s) in {preseeded_allocations} literal(s)"
                ),
                format!(
                    "limits: max array elements {MAX_PRESEED_ARRAY_ELEMENTS}, max object keys {MAX_PRESEED_OBJECT_KEYS}"
                ),
            ],
            input_bytes: source.len(),
        })
    }
}

mod program;
mod object_literals;
pub(crate) use object_literals::fold_descriptor_literals;
pub(crate) use program::simplify_thread_property_writes;
pub use program::{simplify_program_property_writes, simplify_readable_program_property_writes};

/// Rewrite property writes while preserving an `EmittedJavaScript` artifact's
/// statement/node maps. Edits are expression-local, so indexes are unchanged.
pub fn simplify_emitted_property_writes(
    emitted: &EmittedJavaScript,
) -> Result<CleanedEmittedPropertyWrites, PropertyWriteCleanupError> {
    let outcome = simplify_property_writes(&emitted.code)?;
    Ok(CleanedEmittedPropertyWrites {
        code: outcome.code.clone(),
        statement_map: emitted.statement_map.clone(),
        node_map: emitted.node_map.clone(),
        object_parameters: emitted.object_parameters.clone(),
        statement_map_status: StatementMapStatus::Preserved,
        outcome,
    })
}

/// Rewrite property writes while preserving the maps returned by
/// [`super::cleanup_emitted_javascript`].
pub fn simplify_cleaned_emitted_javascript(
    cleaned: &CleanedEmittedJavaScript,
) -> Result<CleanedEmittedPropertyWrites, PropertyWriteCleanupError> {
    let outcome = simplify_property_writes(&cleaned.code)?;
    Ok(CleanedEmittedPropertyWrites {
        code: outcome.code.clone(),
        statement_map: cleaned.statement_map.clone(),
        node_map: cleaned.node_map.clone(),
        object_parameters: cleaned.object_parameters.clone(),
        statement_map_status: StatementMapStatus::Preserved,
        outcome,
    })
}

fn changed_rewrites(
    edits: &mut Vec<analysis::Edit>,
    calls: &[CandidateCall],
    rewrite_by_call: &[Option<String>],
) {
    for (index, replacement) in rewrite_by_call.iter().enumerate() {
        let Some(replacement) = replacement else {
            continue;
        };
        edits.push(analysis::Edit::replace(
            calls[index].span,
            replacement.clone(),
        ));
    }
}

fn set_skip(
    skip_by_call: &mut [Option<PropertyWriteSkip>],
    calls: &[CandidateCall],
    index: usize,
    reason: PropertyWriteSkipReason,
    detail: &str,
) {
    skip_by_call[index] = Some(PropertyWriteSkip {
        span: span_pair(calls[index].span),
        reason,
        detail: detail.to_owned(),
    });
}

fn span_pair(span: Span) -> (u32, u32) {
    (span.start, span.end)
}

// ---------------------------------------------------------------------------
// Native authority
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// AST scanning
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Direct-statement contexts and allocations
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Keys and descriptors
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Symbol planning
// ---------------------------------------------------------------------------

