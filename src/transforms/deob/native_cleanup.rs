//! Binding-aware cleanup of the small native-function catalogue emitted by the
//! Shape kernel minifier.
//!
//! This pass deliberately works from Oxc's semantic `SymbolId`/reference graph.
//! Names such as `Ot` and `Oq` are never treated as meaningful on their own:
//! the initializer, scope, writes, and every use must agree before a rewrite is
//! made.  Replacements are exact source edits followed by an Oxc reparse and
//! codegen, so this module never executes JavaScript.

use std::collections::{BTreeSet, HashMap, HashSet};

use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstKind};
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, ReferenceFlags, Scoping, SymbolId};
use oxc_span::{GetSpan, Ident, Span};

use crate::{
    analysis,
    diagnostics::{Diagnostic, Stage, StageStatus},
};

/// Counts emitted by the native cleanup pass.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NativeCleanupReport {
    pub direct_native_aliases: usize,
    pub member_aliases: usize,
    pub object_indexed_refs: usize,
    pub uncurry_normalized: usize,
    pub skipped: usize,
}

/// Result of running CLEAN's native alias sub-pass.
#[derive(Debug, Clone)]
pub struct NativeCleanupResult {
    pub output: String,
    pub report: NativeCleanupReport,
    pub changed_nodes: usize,
    pub status: StageStatus,
}

/// Controls how native snapshots are treated during cleanup.
///
/// `Conservative` treats writes to native properties as barriers. `Readable`
/// is the presentation policy used by the default deobfuscator: it assumes
/// standard built-in properties remain unchanged, so captured native methods
/// can be printed as stable native reads. Binding reassignment, lexical
/// shadowing, dynamic scope, and first-class function identity remain guarded.
/// Readable additionally treats a dynamic-key `defineProperty` on a
/// member-derived local value as a VM-local property write, because that is the
/// shape the captured VM uses for its own `defineProperty` opcode. Explicit or
/// literal source evidence of a global rebind still blocks the saved identity,
/// and a first-class bound helper keeps its captured declaration (rather than
/// being reallocated through `Function.prototype.call.bind(...)` per use).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeCleanupPolicy {
    Conservative,
    Readable,
}

#[derive(Debug, Clone)]
struct DeclarationInfo {
    span: Span,
    declarators: Vec<Span>,
}

#[derive(Debug, Clone)]
struct Candidate {
    symbol: SymbolId,
    name: String,
    declaration: DeclarationInfo,
    declarator_span: Span,
    raw: RawInit,
}

#[derive(Debug, Clone)]
enum RawInit {
    Expr(RawExpr),
    Conditional {
        test: String,
        consequent: RawExpr,
        alternate: RawExpr,
        roots: BTreeSet<String>,
    },
    FactoryCall {
        call_base: SymbolId,
        bind_target: SymbolId,
        bound_this: SymbolId,
    },
    ApplyFactory {
        factory: SymbolId,
        argument: RawExpr,
    },
    Unknown,
}

#[derive(Debug, Clone)]
enum RawExpr {
    Global(String),
    Alias(SymbolId),
    Member(Box<RawExpr>, String),
    /// A computed member read (`frame.stack[index]`). Opaque for value
    /// resolution and property purity, but distinct from a call result or an
    /// arbitrary expression so the readable defineProperty-target shape can
    /// recognize a value fetched from an object at runtime.
    Computed(Box<RawExpr>),
    Null,
    EmptyArray,
    EmptyObject,
    EmptyString,
    Unknown,
}

/// Outcome of classifying the first argument of an `Object.defineProperty`
/// call. Only `Local` permits *not* recording any barrier: a write to a proven
/// fresh local object or to the `prototype` of a stable fresh function/class
/// cannot reach a native global. `Native` (a native constructor/prototype or a
/// global-object member) records a member barrier for that root, which retains
/// member-derived snapshots but not bare constructor identities. `Opaque` (an
/// unresolved global, parameter, call result, unproven member/computed read, or
/// reassignable binding) records an unproven write that retains every
/// member-derived snapshot. Base locality never proves the pointee's locality.
///
/// The write *key* matters independently of the target class: a target that is
/// the global object (explicitly, through an alias, or because it is opaque and
/// might be `globalThis`) plus a literal native key rebinds that global, and an
/// unknown key can rebind any native global. That is a binding barrier, not a
/// property-stability question, so it applies under both policies. A non-native
/// key on such a target only adds an unrelated global property.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TargetClass {
    Local,
    Native,
    Opaque,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ValueKind {
    Plain,
    PrototypeMethod,
    Factory,
    Uncurried,
    GenericApply,
    Conditional,
    /// A fresh per-evaluation literal value (`[]`, `{}`, `""`).
    ///
    /// Such a value may act as the receiver for a safe member canonicalization
    /// (`[].push` is really `Array.prototype.push`), but it is an identity, not
    /// the global constructor. It must never be replaced by `Array`/`Object`/
    /// `String`, whether named directly or reached through an alias chain.
    FreshLiteral,
}

#[derive(Debug, Clone)]
struct Value {
    replacement: String,
    roots: BTreeSet<String>,
    kind: ValueKind,
}

#[derive(Debug, Clone, Default)]
struct MutationSet {
    /// All native roots under a whole-root barrier: a binding reassignment
    /// (`Array = other`), a plain member assignment (`Math.min = other`, kept
    /// strict in conservative mode), a computed write, or an unconditional
    /// native mutation call. A `defineProperty` call with a proven native
    /// target records only the member barrier in [`Self::member_roots`], so
    /// identity values stay inlinable while member-derived snapshots are
    /// retained. A `defineProperty` on the global object with a native key
    /// records the root here *and* in [`Self::binding_roots`].
    roots: BTreeSet<String>,
    /// Subset of [`Self::roots`] that specifically reassigns a global binding.
    /// Readable mode retains this barrier; a plain member write does not look
    /// like lexical global shadowing. `Object.defineProperty(globalThis,
    /// "Array", ...)` and an unknown key on a possible global target belong
    /// here, because they replace the binding rather than a built-in property.
    binding_roots: BTreeSet<String>,
    /// Native roots whose *members* were written (`Array.prototype.push = evil`,
    /// `Object.defineProperty(Array.prototype, ...)`). A property write cannot
    /// reassign the global it belongs to, so this retains only values derived
    /// from a member of the root, never a bare constructor reference. It also
    /// keeps property writes from looking like lexical global bindings to the
    /// shadow-capture check.
    member_roots: BTreeSet<String>,
    /// Exact canonical member paths written when both object and key are known.
    paths: BTreeSet<String>,
    /// A property write whose target object is unproven: it may land on any
    /// native object, so it retains every member-derived snapshot. The same
    /// write may also be a global rebind; that effect is recorded separately in
    /// [`Self::binding_roots`] according to its key.
    unknown_member_write: bool,
    /// Byte range of the first such unproven write, for diagnostics.
    unknown_member_site: Option<Span>,
    symbols: BTreeSet<SymbolId>,
    member_targets: Vec<MemberMutation>,
    calls: Vec<CallMutation>,
}

#[derive(Debug, Clone)]
struct MemberMutation {
    object_symbol: Option<SymbolId>,
    object_name: Option<String>,
    path: String,
    property: Option<String>,
    /// Span of the written member, used to point at the exact mutation site.
    span: Span,
    /// For a `defineProperty` member call (literal `Object.defineProperty(...)`
    /// or an aliased receiver that resolves to `Object`/`Reflect`), the raw
    /// shape of the target (first argument). `None` for assignments and other
    /// member writes, which keep the unconditional barrier.
    define_target: Option<RawExpr>,
    /// Literal string key of a `defineProperty` call (second argument), when the
    /// key is a plain string literal. `None` for a dynamic key.
    write_key: Option<String>,
}

/// A call whose callee is a bare identifier. When that identifier resolves to a
/// `defineProperty` function the first argument decides whether the call can
/// reach a native global.
#[derive(Debug, Clone)]
struct CallMutation {
    callee: SymbolId,
    define_target: RawExpr,
    span: Span,
    write_key: Option<String>,
}

struct Collector<'a> {
    current: Option<DeclarationInfo>,
    candidates: Vec<Candidate>,
    tables: Vec<TableCandidate>,
    dynamic: bool,
    source: &'a str,
    scoping: &'a Scoping,
}

#[derive(Debug, Clone)]
struct TableCandidate {
    symbol: SymbolId,
    name: String,
    declaration: DeclarationInfo,
    elements: Vec<Option<RawExpr>>,
}

struct OperationTableCollector<'a> {
    scoping: &'a Scoping,
    function_tables: HashMap<SymbolId, Vec<Span>>,
    called_tables: HashSet<SymbolId>,
}

struct MutationCollector<'a> {
    dynamic: bool,
    mutations: MutationSet,
    scoping: &'a Scoping,
}

#[derive(Debug, Clone)]
struct Replacement {
    span: Span,
    text: String,
}

/// Run the native alias cleanup. The returned source is always reparsed and
/// emitted by Oxc; a malformed edit therefore becomes a normal pipeline error.
pub fn run(source: &str, diagnostics: &mut Vec<Diagnostic>) -> Result<NativeCleanupResult, String> {
    run_with_policy(source, diagnostics, NativeCleanupPolicy::Conservative)
}

/// Run native cleanup under the readable presentation policy.
pub fn run_readable(
    source: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<NativeCleanupResult, String> {
    run_with_policy(source, diagnostics, NativeCleanupPolicy::Readable)
}

/// Run native cleanup with an explicit mutation policy.
pub fn run_with_policy(
    source: &str,
    diagnostics: &mut Vec<Diagnostic>,
    policy: NativeCleanupPolicy,
) -> Result<NativeCleanupResult, String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut collector = Collector {
        current: None,
        candidates: Vec::new(),
        tables: Vec::new(),
        dynamic: false,
        source,
        scoping: &scoping,
    };
    collector.visit_program(&program);
    collector.dynamic |= has_dynamic_scope(&program, &scoping);

    let operation_function_spans = proven_operation_function_spans(&program, &scoping);

    let mut mutation_collector = MutationCollector {
        dynamic: false,
        mutations: MutationSet::default(),
        scoping: &scoping,
    };
    mutation_collector.visit_program(&program);
    collector.dynamic |= mutation_collector.dynamic;
    let mut mutations = mutation_collector.mutations;
    if policy == NativeCleanupPolicy::Readable {
        diagnostics.push(Diagnostic::info(
            Stage::Clean,
            "NATIVE_READABLE_ASSUMPTION",
            "readable native cleanup assumes standard built-in properties remain unchanged; captured native methods are inlined",
        ));
    }
    if collector.dynamic {
        diagnostics.push(Diagnostic::warn(
            Stage::Clean,
            "DYNAMIC_SCOPE",
            "with or eval (direct or indirect) prevents alias cleanup; source retained",
        ));
        diagnostics.push(Diagnostic::warn(
            Stage::Clean,
            "NATIVE_ALIAS_DYNAMIC_SCOPE",
            "with or eval (direct or indirect) prevents proving native aliases across lexical scope",
        ));
        return Ok(NativeCleanupResult {
            output: source.to_owned(),
            report: NativeCleanupReport::default(),
            changed_nodes: 0,
            status: StageStatus::Unknown,
        });
    }

    let mut values = HashMap::new();
    let raw_by_symbol: HashMap<SymbolId, RawInit> = collector
        .candidates
        .iter()
        .map(|candidate| (candidate.symbol, candidate.raw.clone()))
        .collect();
    let mut resolving = HashSet::new();
    for symbol in raw_by_symbol.keys().copied().collect::<Vec<_>>() {
        let _ = resolve_symbol(symbol, &raw_by_symbol, &mut values, &mut resolving);
    }
    expand_mutations(&mut mutations, &values, &raw_by_symbol, &scoping, policy);

    let mut edits = Vec::new();
    let mut covered = Vec::new();
    let mut declaration_spans = Vec::new();
    let mut removed_by_declaration: HashMap<(u32, u32), (DeclarationInfo, Vec<Span>)> =
        HashMap::new();
    let mut report = NativeCleanupReport::default();
    if program
        .body
        .first()
        .is_some_and(|statement| matches!(statement, Statement::EmptyStatement(_)))
    {
        edits.push(analysis::Edit::delete(program.body[0].span()));
        diagnostics.push(Diagnostic::info(
            Stage::Clean,
            "EMPTY_STATEMENT",
            "removed leading empty statement",
        ));
    }
    let factory_calls = collect_factory_calls(&program, &scoping, &nodes, &values);
    let protected_symbols = protected_uncurry_dependencies(
        &collector.candidates,
        &raw_by_symbol,
        &values,
        &factory_calls,
        &scoping,
        &nodes,
        policy,
    );
    let derived_alias_declarations: Vec<Span> = collector
        .candidates
        .iter()
        .filter(|candidate| {
            values.get(&candidate.symbol).is_some_and(|value| {
                matches!(value.kind, ValueKind::Uncurried | ValueKind::GenericApply)
                    && !protected_symbols.contains(&candidate.symbol)
            })
        })
        // Only the derived alias's own declarator is factory-owned. Using the
        // enclosing declaration span would also skip unrelated references in a
        // shared `var A = ..., B = ...;` statement and could remove `A` while
        // leaving a dangling `B = A.foo` initializer behind.
        .map(|candidate| candidate.declarator_span)
        .collect();
    for replacement in &factory_calls {
        edits.push(analysis::Edit::replace(
            replacement.span,
            replacement.text.clone(),
        ));
        covered.push(replacement.span);
        report.uncurry_normalized += 1;
    }

    for candidate in &collector.candidates {
        let Some(value) = values.get(&candidate.symbol) else {
            continue;
        };
        if protected_symbols.contains(&candidate.symbol) {
            report.skipped += 1;
            diagnostics.push(native_skip(
                candidate,
                "retained dependency of a first-class uncurry value",
            ));
            continue;
        }
        if policy == NativeCleanupPolicy::Conservative
            && value.kind == ValueKind::Factory
            && !factory_calls_cover_symbol(
                candidate.symbol,
                &factory_calls,
                &derived_alias_declarations,
                &scoping,
                &nodes,
            )
        {
            report.skipped += 1;
            diagnostics.push(native_skip(
                candidate,
                "uncurry factory has a use outside a proven call",
            ));
            continue;
        }
        if scoping.symbol_is_mutated(candidate.symbol) {
            report.skipped += 1;
            diagnostics.push(native_skip(candidate, "binding is reassigned"));
            continue;
        }
        if policy == NativeCleanupPolicy::Conservative {
            if let Some(cause) = value_mutation_cause(value, &mutations) {
                report.skipped += 1;
                diagnostics.push(native_mutation_skip(candidate, value, &mutations, cause));
                continue;
            }
        } else if value_is_mutated(value, &mutations, policy) {
            report.skipped += 1;
            diagnostics.push(native_skip(
                candidate,
                "binding or native receiver is mutated",
            ));
            continue;
        }
        let has_read = scoping
            .get_resolved_references(candidate.symbol)
            .any(|reference| reference.flags().contains(ReferenceFlags::Read));
        if !has_read {
            if unused_alias_is_pure(candidate, value, &mutations, policy) {
                removed_by_declaration
                    .entry((
                        candidate.declaration.span.start,
                        candidate.declaration.span.end,
                    ))
                    .or_insert_with(|| (candidate.declaration.clone(), Vec::new()))
                    .1
                    .push(candidate.declarator_span);
                diagnostics.push(
                    Diagnostic::info(
                        Stage::Clean,
                        "NATIVE_ALIAS_REMOVED",
                        format!("removed unused native alias `{}`", candidate.name),
                    )
                    .with_span(
                        candidate.declarator_span.start,
                        candidate.declarator_span.end,
                    ),
                );
            } else {
                report.skipped += 1;
                diagnostics.push(native_skip(
                    candidate,
                    "unused alias retained to preserve its initializer",
                ));
            }
            continue;
        }
        // A binding initialized by a fresh literal (`[]`, `{}`, `""`) holds a
        // new per-evaluation value, not the global constructor. Member reads on
        // it still canonicalize (handled through the resolved `Value`), but the
        // value itself has identity and must be retained. This also covers
        // transitive aliases (`B = A`) and catalogue elements, because they
        // resolve to the same `FreshLiteral` value through `resolve_symbol` /
        // `resolve_table_expr`.
        if value.kind == ValueKind::FreshLiteral {
            report.skipped += 1;
            diagnostics.push(native_skip(
                candidate,
                "fresh literal value has identity and is not a global constructor",
            ));
            continue;
        }
        let mut safe = true;
        let mut rewrites = Vec::new();
        for reference in scoping.get_resolved_references(candidate.symbol) {
            if !reference.flags().contains(ReferenceFlags::Read) {
                continue;
            }
            let Some(AstKind::IdentifierReference(identifier)) =
                Some(nodes.kind(reference.node_id()))
            else {
                continue;
            };
            if covered.iter().any(|span| contains(*span, identifier.span)) {
                continue;
            }
            if derived_alias_declarations
                .iter()
                .any(|declaration| contains(*declaration, identifier.span))
            {
                continue;
            }
            if (policy == NativeCleanupPolicy::Conservative && value.kind == ValueKind::Factory)
                || reference_captures_native(&scoping, value, reference, &mutations)
            {
                safe = false;
                break;
            }
            if policy == NativeCleanupPolicy::Conservative
                && matches!(value.kind, ValueKind::Uncurried | ValueKind::GenericApply)
                && !matches!(
                    nodes.parent_kind(reference.node_id()),
                    AstKind::CallExpression(call) if contains(call.callee.span(), identifier.span)
                )
            {
                safe = false;
                break;
            }
            if value.kind == ValueKind::PrototypeMethod
                && matches!(
                    nodes.parent_kind(reference.node_id()),
                    AstKind::CallExpression(call) if contains(call.callee.span(), identifier.span)
                )
            {
                safe = false;
                break;
            }
            if matches!(nodes.parent_kind(reference.node_id()), AstKind::ObjectProperty(property) if property.shorthand)
            {
                safe = false;
                break;
            }
            let direct_callee = matches!(
                nodes.parent_kind(reference.node_id()),
                AstKind::CallExpression(call) if contains(call.callee.span(), identifier.span)
            );
            rewrites.push(Replacement {
                span: identifier.span,
                text: if direct_callee {
                    replacement_text(value)
                } else {
                    reference_replacement_text(value, policy)
                },
            });
        }
        if !safe {
            report.skipped += 1;
            diagnostics.push(native_skip(
                candidate,
                "a use would capture a shadow or shorthand key",
            ));
            continue;
        }
        if value.kind == ValueKind::PrototypeMethod
            || value.kind == ValueKind::Uncurried
            || value.kind == ValueKind::GenericApply
        {
            report.member_aliases += 1;
        } else {
            report.direct_native_aliases += 1;
        }
        for rewrite in rewrites {
            edits.push(analysis::Edit::replace(rewrite.span, rewrite.text));
            covered.push(rewrite.span);
        }
        removed_by_declaration
            .entry((
                candidate.declaration.span.start,
                candidate.declaration.span.end,
            ))
            .or_insert_with(|| (candidate.declaration.clone(), Vec::new()))
            .1
            .push(candidate.declarator_span);
        diagnostics.push(
            Diagnostic::info(
                Stage::Clean,
                "NATIVE_ALIAS_INLINED",
                format!("inlined `{}` as `{}`", candidate.name, value.replacement),
            )
            .with_span(
                candidate.declarator_span.start,
                candidate.declarator_span.end,
            ),
        );
    }

    // An unused fresh-empty-array local (`var Bs = [];`) carries no identity
    // and no side effect. Removing it is safe only when the binding has no
    // references at all and the declaration is an ordinary statement, never a
    // for/for-in initializer. This is deliberately narrower than general dead
    // store elimination; the initializer must be the empty array literal.
    for node in nodes.iter() {
        let AstKind::VariableDeclaration(declaration) = node.kind() else {
            continue;
        };
        if !matches!(
            nodes.parent_kind(node.id()),
            AstKind::Program(_)
                | AstKind::FunctionBody(_)
                | AstKind::BlockStatement(_)
                | AstKind::StaticBlock(_)
                | AstKind::SwitchCase(_)
        ) {
            continue;
        }
        for declarator in &declaration.declarations {
            let Some(binding) = simple_binding(&declarator.id) else {
                continue;
            };
            let Some(symbol) = binding.symbol_id.get() else {
                continue;
            };
            let Some(Expression::ArrayExpression(array)) = declarator.init.as_ref() else {
                continue;
            };
            if !array.elements.is_empty() || !scoping.symbol_is_unused(symbol) {
                continue;
            }
            // The retained-alias loop may already have proven this exact
            // declarator removable; do not report or remove it twice.
            if removed_by_declaration
                .values()
                .any(|(_, removed)| removed.contains(&declarator.span))
            {
                continue;
            }
            removed_by_declaration
                .entry((declaration.span.start, declaration.span.end))
                .or_insert_with(|| {
                    (
                        DeclarationInfo {
                            span: declaration.span,
                            declarators: declaration
                                .declarations
                                .iter()
                                .map(|declarator| declarator.span)
                                .collect(),
                        },
                        Vec::new(),
                    )
                })
                .1
                .push(declarator.span);
            diagnostics.push(
                Diagnostic::info(
                    Stage::Clean,
                    "UNUSED_EMPTY_ARRAY_REMOVED",
                    format!("removed unused empty-array local `{}`", binding.name),
                )
                .with_span(declarator.span.start, declarator.span.end),
            );
        }
    }

    for (declaration, removed) in removed_by_declaration.into_values() {
        for span in declaration_removals(&declaration, &removed) {
            declaration_spans.push(span);
            edits.push(analysis::Edit::delete(span));
        }
    }

    let table_edits = clean_native_tables(
        source,
        &program,
        &collector.tables,
        &scoping,
        &nodes,
        &raw_by_symbol,
        &values,
        &mutations,
        &operation_function_spans,
        &mut report,
        diagnostics,
    );
    for edit in table_edits.edits {
        covered.push(edit_span(&edit));
        edits.push(edit);
    }
    for span in table_edits.declarations {
        declaration_spans.push(span);
        edits.push(analysis::Edit::delete(span));
    }

    // A declaration deletion also removes alias references in its initializer.
    // Dropping nested edits avoids the classic `var A=..., B=...` overlap bug.
    edits.retain(|edit| {
        !declaration_spans
            .iter()
            .any(|span| *span != edit_span(edit) && contains(*span, edit_span(edit)))
    });
    let mut changed = 0;
    let mut next = source.to_owned();
    if !edits.is_empty() {
        let applied = analysis::apply_edits(source, edits);
        if !applied.rejected.is_empty() {
            diagnostics.push(Diagnostic::warn(
                Stage::Clean,
                "NATIVE_EDIT_OVERLAP",
                analysis::describe_rejected_edits(&applied.rejected),
            ));
        }
        changed = applied.applied;
        next = applied.output;
    }
    let output = reparse_codegen(&next)?;
    if collector.dynamic {
        diagnostics.push(Diagnostic::warn(
            Stage::Clean,
            "NATIVE_ALIAS_RETAINED",
            "native aliases retained where dynamic scope prevents a binding proof",
        ));
    }
    Ok(NativeCleanupResult {
        output,
        report,
        changed_nodes: changed,
        status: if changed == 0 {
            StageStatus::Unknown
        } else {
            StageStatus::Found
        },
    })
}

fn edit_span(edit: &analysis::Edit) -> Span {
    Span::new(edit.start, edit.end)
}

fn replacement_text(value: &Value) -> String {
    if value.kind == ValueKind::Conditional {
        format!("({})", value.replacement)
    } else {
        value.replacement.clone()
    }
}

fn reference_replacement_text(value: &Value, policy: NativeCleanupPolicy) -> String {
    if policy == NativeCleanupPolicy::Readable
        && matches!(value.kind, ValueKind::Uncurried | ValueKind::GenericApply)
    {
        return first_class_uncurried_text(value);
    }
    replacement_text(value)
}

fn first_class_uncurried_text(value: &Value) -> String {
    match value.kind {
        ValueKind::GenericApply => "Function.prototype.call.bind(Function.prototype.apply)".into(),
        ValueKind::Uncurried => {
            let method = value
                .replacement
                .strip_suffix(".call")
                .unwrap_or(value.replacement.as_str());
            format!("Function.prototype.call.bind({method})")
        }
        _ => replacement_text(value),
    }
}

fn native_skip(candidate: &Candidate, reason: &'static str) -> Diagnostic {
    Diagnostic::warn(
        Stage::Clean,
        "NATIVE_ALIAS_SKIPPED",
        format!("retained `{}`: {reason}", candidate.name),
    )
    .with_span(
        candidate.declarator_span.start,
        candidate.declarator_span.end,
    )
}

/// Explain exactly which native mutation blocked inlining `candidate`, naming
/// the affected builtin and (for an unproven write) the write site.
fn native_mutation_skip(
    candidate: &Candidate,
    value: &Value,
    mutations: &MutationSet,
    cause: MutationCause,
) -> Diagnostic {
    let affected: Vec<&str> = value
        .roots
        .iter()
        .filter(|root| mutations.member_roots.contains(*root) || mutations.roots.contains(*root))
        .map(String::as_str)
        .collect();
    let reason = match cause {
        MutationCause::Root => format!(
            "native binding `{}` is reassigned",
            if affected.is_empty() {
                value.replacement.as_str()
            } else {
                affected[0]
            }
        ),
        MutationCause::Member => format!(
            "native member of `{}` is written",
            if affected.is_empty() {
                value.replacement.as_str()
            } else {
                affected[0]
            }
        ),
        MutationCause::UnknownMember => format!(
            "a property write{} on an unproven target may overwrite a native member",
            mutations
                .unknown_member_site
                .map_or(String::new(), |span| format!(
                    " at {}",
                    crate::analysis::span_label(span)
                ))
        ),
    };
    Diagnostic::warn(
        Stage::Clean,
        "NATIVE_ALIAS_SKIPPED",
        format!("retained `{}`: {reason}", candidate.name),
    )
    .with_span(
        candidate.declarator_span.start,
        candidate.declarator_span.end,
    )
}

fn unused_alias_is_pure(
    candidate: &Candidate,
    value: &Value,
    mutations: &MutationSet,
    policy: NativeCleanupPolicy,
) -> bool {
    let initializer_is_pure = match &candidate.raw {
        RawInit::FactoryCall { .. } | RawInit::ApplyFactory { .. } => true,
        RawInit::Expr(expression) => raw_expr_is_pure(expression),
        RawInit::Conditional { .. } | RawInit::Unknown => false,
    };
    initializer_is_pure && !value_is_mutated(value, mutations, policy)
}

fn raw_expr_is_pure(expression: &RawExpr) -> bool {
    match expression {
        RawExpr::EmptyArray | RawExpr::EmptyObject | RawExpr::EmptyString => true,
        RawExpr::Member(base, _) => raw_expr_is_pure(base),
        RawExpr::Computed(_) => false,
        RawExpr::Global(name) => analysis::is_native_global(name),
        RawExpr::Null => true,
        RawExpr::Alias(_) | RawExpr::Unknown => false,
    }
}

fn contains(outer: Span, inner: Span) -> bool {
    outer.start <= inner.start && outer.end >= inner.end
}

fn declaration_removals(declaration: &DeclarationInfo, removed: &[Span]) -> Vec<Span> {
    let removed: BTreeSet<_> = removed.iter().copied().collect();
    if removed.len() == declaration.declarators.len() {
        return vec![declaration.span];
    }
    let mut spans = Vec::new();
    let mut index = 0;
    while index < declaration.declarators.len() {
        if !removed.contains(&declaration.declarators[index]) {
            index += 1;
            continue;
        }
        let start = index;
        while index + 1 < declaration.declarators.len()
            && removed.contains(&declaration.declarators[index + 1])
        {
            index += 1;
        }
        let end = index;
        let span = if start == 0 {
            Span::new(
                declaration.declarators[start].start,
                declaration.declarators[end + 1].start,
            )
        } else {
            Span::new(
                declaration.declarators[start - 1].end,
                declaration.declarators[end].end,
            )
        };
        spans.push(span);
        index += 1;
    }
    spans
}

fn resolve_symbol(
    symbol: SymbolId,
    raw: &HashMap<SymbolId, RawInit>,
    values: &mut HashMap<SymbolId, Value>,
    resolving: &mut HashSet<SymbolId>,
) -> Option<Value> {
    if let Some(value) = values.get(&symbol) {
        return Some(value.clone());
    }
    if !resolving.insert(symbol) {
        return None;
    }
    let value = match raw.get(&symbol)? {
        RawInit::Expr(expression) => resolve_raw(expression, raw, values, resolving),
        RawInit::Conditional {
            test,
            consequent,
            alternate,
            roots,
        } => {
            let consequent = resolve_raw(consequent, raw, values, resolving)?;
            let alternate = resolve_raw(alternate, raw, values, resolving)?;
            if consequent.kind == ValueKind::FreshLiteral
                || alternate.kind == ValueKind::FreshLiteral
            {
                None
            } else {
                let mut roots = roots.clone();
                roots.extend(consequent.roots.iter().cloned());
                roots.extend(alternate.roots.iter().cloned());
                Some(Value {
                    replacement: format!(
                        "{} ? {} : {}",
                        test,
                        replacement_text(&consequent),
                        replacement_text(&alternate)
                    ),
                    roots,
                    kind: ValueKind::Conditional,
                })
            }
        }
        RawInit::FactoryCall {
            call_base,
            bind_target,
            bound_this,
        } => {
            let base = resolve_symbol(*call_base, raw, values, resolving)?;
            let target = resolve_symbol(*bind_target, raw, values, resolving)?;
            let this_value = resolve_symbol(*bound_this, raw, values, resolving)?;
            (base.replacement == "Function.prototype.call"
                && target.replacement == "Function.prototype.bind"
                && this_value.replacement == "Function.prototype.call")
                .then_some(Value {
                    replacement: "Function.prototype.bind.bind(Function.prototype.call)".into(),
                    roots: ["Function".into()].into_iter().collect(),
                    kind: ValueKind::Factory,
                })
        }
        RawInit::ApplyFactory { factory, argument } => {
            let factory_value = resolve_symbol(*factory, raw, values, resolving)?;
            if factory_value.kind != ValueKind::Factory {
                None
            } else {
                let argument_value = resolve_raw(argument, raw, values, resolving)?;
                let kind = if argument_value.replacement == "Function.prototype.apply" {
                    ValueKind::GenericApply
                } else if argument_value.kind == ValueKind::PrototypeMethod {
                    ValueKind::Uncurried
                } else {
                    ValueKind::Plain
                };
                (kind != ValueKind::Plain).then_some(Value {
                    replacement: if kind == ValueKind::GenericApply {
                        "Function.prototype.apply.call".into()
                    } else {
                        format!("{}.call", argument_value.replacement)
                    },
                    roots: argument_value.roots,
                    kind,
                })
            }
        }
        RawInit::Unknown => None,
    }?;
    resolving.remove(&symbol);
    values.insert(symbol, value.clone());
    Some(value)
}

fn resolve_raw(
    expression: &RawExpr,
    raw: &HashMap<SymbolId, RawInit>,
    values: &mut HashMap<SymbolId, Value>,
    resolving: &mut HashSet<SymbolId>,
) -> Option<Value> {
    match expression {
        RawExpr::Global(name) if analysis::is_native_global(name) => Some(Value {
            replacement: name.clone(),
            roots: [name.clone()].into_iter().collect(),
            kind: ValueKind::Plain,
        }),
        RawExpr::Global(_) => None,
        RawExpr::Alias(symbol) => resolve_symbol(*symbol, raw, values, resolving),
        RawExpr::Member(base, property) => {
            let base = resolve_raw(base, raw, values, resolving)?;
            canonical_member(&base, property)
        }
        RawExpr::Null => Some(Value {
            replacement: "null".into(),
            roots: BTreeSet::new(),
            kind: ValueKind::Plain,
        }),
        RawExpr::EmptyArray => Some(Value {
            replacement: "Array".into(),
            roots: ["Array".into()].into_iter().collect(),
            kind: ValueKind::FreshLiteral,
        }),
        RawExpr::EmptyObject => Some(Value {
            replacement: "Object".into(),
            roots: ["Object".into()].into_iter().collect(),
            kind: ValueKind::FreshLiteral,
        }),
        RawExpr::EmptyString => Some(Value {
            replacement: "String".into(),
            roots: ["String".into()].into_iter().collect(),
            kind: ValueKind::FreshLiteral,
        }),
        RawExpr::Computed(_) | RawExpr::Unknown => None,
    }
}

/// Resolve a raw expression against an already-complete value table.
///
/// `resolve_raw` can discover and cache new symbol values while walking; the
/// caller here has already resolved every catalogue candidate once, so alias
/// lookup is a read. This is the same resolution as `resolve_raw` for an
/// expression whose reachable bindings are all in `values` (unresolvable ones
/// remain `None`), without cloning the whole table per expression.
fn resolve_raw_frozen(expression: &RawExpr, values: &HashMap<SymbolId, Value>) -> Option<Value> {
    match expression {
        RawExpr::Global(name) if analysis::is_native_global(name) => Some(Value {
            replacement: name.clone(),
            roots: [name.clone()].into_iter().collect(),
            kind: ValueKind::Plain,
        }),
        RawExpr::Global(_) => None,
        RawExpr::Alias(symbol) => values.get(symbol).cloned(),
        RawExpr::Member(base, property) => {
            canonical_member(&resolve_raw_frozen(base, values)?, property)
        }
        RawExpr::Null => Some(Value {
            replacement: "null".into(),
            roots: BTreeSet::new(),
            kind: ValueKind::Plain,
        }),
        RawExpr::EmptyArray => Some(Value {
            replacement: "Array".into(),
            roots: ["Array".into()].into_iter().collect(),
            kind: ValueKind::FreshLiteral,
        }),
        RawExpr::EmptyObject => Some(Value {
            replacement: "Object".into(),
            roots: ["Object".into()].into_iter().collect(),
            kind: ValueKind::FreshLiteral,
        }),
        RawExpr::EmptyString => Some(Value {
            replacement: "String".into(),
            roots: ["String".into()].into_iter().collect(),
            kind: ValueKind::FreshLiteral,
        }),
        RawExpr::Computed(_) | RawExpr::Unknown => None,
    }
}

fn canonical_member(base: &Value, property: &str) -> Option<Value> {
    let mut roots = base.roots.clone();
    let (replacement, kind) = match (base.replacement.as_str(), property) {
        ("Object", "bind") => ("Function.prototype.bind", ValueKind::Plain),
        ("Object", "call") => ("Function.prototype.call", ValueKind::Plain),
        ("Object", "apply") => ("Function.prototype.apply", ValueKind::Plain),
        ("Object", "prototype") => ("Object.prototype", ValueKind::Plain),
        ("Array", "prototype") => ("Array.prototype", ValueKind::Plain),
        ("String", "prototype") => ("String.prototype", ValueKind::Plain),
        ("Function", "prototype") => ("Function.prototype", ValueKind::Plain),
        ("Object", "hasOwnProperty") => (
            "Object.prototype.hasOwnProperty",
            ValueKind::PrototypeMethod,
        ),
        ("Object", "getOwnPropertyDescriptor") => {
            ("Object.getOwnPropertyDescriptor", ValueKind::Plain)
        }
        ("Object", "defineProperty") => ("Object.defineProperty", ValueKind::Plain),
        // An identifier alias to `Reflect.defineProperty` must reach the same
        // target-classified barrier as the direct and receiver-alias forms.
        ("Reflect", "defineProperty") => ("Reflect.defineProperty", ValueKind::Plain),
        ("Object", "create") => ("Object.create", ValueKind::Plain),
        ("String", "fromCharCode") => ("String.fromCharCode", ValueKind::Plain),
        ("JSON", "stringify") => ("JSON.stringify", ValueKind::Plain),
        ("Math", "min") => ("Math.min", ValueKind::Plain),
        ("Math", "floor") => ("Math.floor", ValueKind::Plain),
        ("URL", "createObjectURL") => ("URL.createObjectURL", ValueKind::Plain),
        ("Object.prototype", "hasOwnProperty") => (
            "Object.prototype.hasOwnProperty",
            ValueKind::PrototypeMethod,
        ),
        ("Array", "push") | ("Array.prototype", "push") => {
            ("Array.prototype.push", ValueKind::PrototypeMethod)
        }
        ("Array", "pop") | ("Array.prototype", "pop") => {
            ("Array.prototype.pop", ValueKind::PrototypeMethod)
        }
        ("Array", "slice") | ("Array.prototype", "slice") => {
            ("Array.prototype.slice", ValueKind::PrototypeMethod)
        }
        ("Array", "splice") | ("Array.prototype", "splice") => {
            ("Array.prototype.splice", ValueKind::PrototypeMethod)
        }
        ("Array", "join") | ("Array.prototype", "join") => {
            ("Array.prototype.join", ValueKind::PrototypeMethod)
        }
        ("Array", "map") | ("Array.prototype", "map") => {
            ("Array.prototype.map", ValueKind::PrototypeMethod)
        }
        ("String", "indexOf") | ("String.prototype", "indexOf") => {
            ("String.prototype.indexOf", ValueKind::PrototypeMethod)
        }
        ("String", "charAt") | ("String.prototype", "charAt") => {
            ("String.prototype.charAt", ValueKind::PrototypeMethod)
        }
        ("Uint8Array", "from") => ("Uint8Array.from", ValueKind::Plain),
        _ => return None,
    };
    if replacement.starts_with("Function.prototype") {
        roots.insert("Function".into());
    }
    Some(Value {
        replacement: replacement.into(),
        roots,
        kind,
    })
}

/// Why a resolved native value can no longer be inlined.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MutationCause {
    /// A binding reassignment, or a whole-native barrier that leaves the
    /// affected root unusable as a constructor identity.
    Root,
    /// A member of a native object the value derives from was written.
    Member,
    /// A property write on an unproven target may have overwritten the member.
    UnknownMember,
}

/// True when the value denotes a bare constructor/global identity plus an
/// optional `typeof`-guarded selection of such identities.
///
/// A `defineProperty` call with a proven native target records only a member
/// barrier, so it does not invalidate such a value: writing a property of
/// `Array.prototype` cannot change which global `Array` resolves to. A plain
/// member assignment (`Math.min = other`) keeps the stricter conservative
/// whole-root barrier, so it still blocks the identity.
fn value_is_identity_only(value: &Value) -> bool {
    matches!(value.kind, ValueKind::Plain | ValueKind::Conditional)
        && !value.replacement.contains('.')
}

fn value_mutation_cause(value: &Value, mutations: &MutationSet) -> Option<MutationCause> {
    let hits_root = value
        .roots
        .iter()
        .any(|root| mutations.roots.contains(root));
    if hits_root {
        // A `defineProperty` on a proven native target records only a member
        // barrier, so it does not appear in `roots`; a plain member assignment
        // keeps the stricter conservative whole-root barrier (and is reported
        // as a member write). A binding reassignment or a whole-native barrier
        // is a root cause.
        if value
            .roots
            .iter()
            .any(|root| mutations.binding_roots.contains(root))
        {
            return Some(MutationCause::Root);
        }
        if value
            .roots
            .iter()
            .any(|root| mutations.member_roots.contains(root))
        {
            return Some(MutationCause::Member);
        }
        return Some(MutationCause::Root);
    }
    if value_is_identity_only(value) {
        return None;
    }
    if value
        .roots
        .iter()
        .any(|root| mutations.member_roots.contains(root))
        || mutations.paths.contains(&value.replacement)
    {
        return Some(MutationCause::Member);
    }
    if mutations.unknown_member_write {
        return Some(MutationCause::UnknownMember);
    }
    None
}

fn value_is_mutated(value: &Value, mutations: &MutationSet, policy: NativeCleanupPolicy) -> bool {
    match policy {
        NativeCleanupPolicy::Conservative => value_mutation_cause(value, mutations).is_some(),
        NativeCleanupPolicy::Readable => {
            value
                .roots
                .iter()
                .any(|root| mutations.binding_roots.contains(root))
                || (!value_is_identity_only(value) && mutations.unknown_member_write)
        }
    }
}

fn expand_mutations(
    mutations: &mut MutationSet,
    values: &HashMap<SymbolId, Value>,
    raw: &HashMap<SymbolId, RawInit>,
    scoping: &Scoping,
    policy: NativeCleanupPolicy,
) {
    // A `X.prototype = ...` write replaces the fresh prototype of a function or
    // class binding, so it can no longer be treated as a local receiver.
    let prototype_written: HashSet<SymbolId> = mutations
        .member_targets
        .iter()
        .filter(|target| target.property.as_deref() == Some("prototype"))
        .filter_map(|target| target.object_symbol)
        .collect();
    let targets = mutations.member_targets.clone();
    let mut fresh_shadows = Vec::new();
    for target in targets {
        let value = target.object_symbol.and_then(|symbol| values.get(&symbol));
        if let Some(value) = value {
            if value.kind == ValueKind::FreshLiteral {
                // A write through a fresh-literal receiver (`A.push = evil`
                // where `A = []`) creates an own property on that fresh value.
                // It shadows only the canonical member with the same name, so
                // record that exact prototype path when the property names one.
                // A write that does not name a known member, or a computed
                // write, cannot be proven to shadow a canonical read, so it is
                // left as the fresh binding's own path: the literal is not the
                // global constructor and must not be re-rooted onto it.
                if let Some(property) = target.property.as_deref() {
                    if let Some(member) = canonical_member(value, property) {
                        fresh_shadows.push(member.replacement);
                    }
                }
                continue;
            }
        }
        let path = value
            .map(|value| {
                target
                    .object_name
                    .as_ref()
                    .and_then(|name| {
                        target
                            .path
                            .strip_prefix(name)
                            .map(|suffix| format!("{}{}", value.replacement, suffix))
                    })
                    .unwrap_or_else(|| {
                        target.property.as_ref().map_or_else(
                            || value.replacement.clone(),
                            |property| format!("{}.{}", value.replacement, property),
                        )
                    })
            })
            .unwrap_or_else(|| target.path.clone());
        let define_call = matches!(
            path.as_str(),
            "Object.defineProperty" | "Reflect.defineProperty"
        )
        .then(|| target.define_target.as_ref())
        .flatten();
        if let Some(define_target) = define_call {
            // `defineProperty` only mutates the object named by its first
            // argument. A proven-local object/prototype cannot reach a native
            // global, so it must not retain any native. A native or opaque
            // target records a *member* barrier: the call may overwrite a
            // property of a native object, but it cannot reassign the global
            // constructor binding that object was reached through. A write that
            // does not come from a `defineProperty` call (e.g.
            // `Object.defineProperty = evil`, which leaves `define_target`
            // unset) is an ordinary member write.
            let mut visited = HashSet::new();
            let locally_owned = classify_define_target(
                define_target,
                raw,
                values,
                scoping,
                &prototype_written,
                &mut visited,
            ) == TargetClass::Local;
            if !locally_owned {
                record_define_member_write(
                    mutations,
                    define_target,
                    target.write_key.as_deref(),
                    values,
                    raw,
                    scoping,
                    policy,
                    target.span,
                );
            }
        } else {
            record_mutation_path(mutations, &path);
        }
    }
    for path in fresh_shadows {
        record_mutation_path(mutations, &path);
    }
    for call in mutations.calls.clone() {
        let Some(value) = values.get(&call.callee) else {
            continue;
        };
        if matches!(
            value.replacement.as_str(),
            "Object.defineProperty" | "Reflect.defineProperty"
        ) {
            let mut visited = HashSet::new();
            if classify_define_target(
                &call.define_target,
                raw,
                values,
                scoping,
                &prototype_written,
                &mut visited,
            ) != TargetClass::Local
            {
                record_define_member_write(
                    mutations,
                    &call.define_target,
                    call.write_key.as_deref(),
                    values,
                    raw,
                    scoping,
                    policy,
                    call.span,
                );
            }
        } else if value.replacement.ends_with(".__defineGetter__")
            || value.replacement.ends_with(".__defineSetter__")
        {
            record_mutation_path(mutations, &value.replacement);
        }
    }
}

/// True when `name` denotes the global object itself. These three names are
/// interchangeable at the top level (`globalThis === window === self` in a
/// browser, and `globalThis === self` in a worker), and `globalThis.globalThis`
/// is the global object again.
fn is_global_object_name(name: &str) -> bool {
    matches!(name, "globalThis" | "window" | "self")
}

/// True when a `defineProperty` target expression is provably the global object
/// (directly, through an alias chain, or through a self-referential member such
/// as `globalThis.globalThis`). An unproven expression is handled by the
/// caller's opaque branch, which treats it as a possible global object too.
fn expression_is_global_object(target: &RawExpr, values: &HashMap<SymbolId, Value>) -> bool {
    match target {
        RawExpr::Global(name) => is_global_object_name(name),
        // `values` resolves an alias chain to its terminal replacement, so an
        // alias to `globalThis`/`window`/`self` is recognized without walking
        // the raw graph again.
        RawExpr::Alias(symbol) => values
            .get(symbol)
            .is_some_and(|value| is_global_object_name(&value.replacement)),
        RawExpr::Member(base, property) => {
            is_global_object_name(property) && expression_is_global_object(base, values)
        }
        _ => false,
    }
}

/// Record the effect of `defineProperty`/`Reflect.defineProperty` on the global
/// object. A literal native key replaces that global binding; a dynamic key can
/// name any native global. A non-native key adds an unrelated global property
/// and cannot change a native identity, so it records nothing.
fn record_global_define_key(mutations: &mut MutationSet, key: Option<&str>) {
    match key {
        Some(key) if analysis::is_native_global(key) => {
            mutations.roots.insert(key.into());
            mutations.binding_roots.insert(key.into());
        }
        Some(_) => {}
        None => record_all_native_mutation(mutations),
    }
}

/// Record a property write whose target is not proven local.
///
/// A write through the global object can reassign any global binding. A write
/// whose target belongs to a known native root affects only that root's
/// members. Everything else is an unproven write that may land on any native
/// member *and* may be the global object: a native literal key then rebinds that
/// global, and a dynamic key may rebind any native global.
///
/// Under `Readable`, one unproven shape is narrowed deliberately: a **dynamic
/// key** on a **member-derived local value** (a non-reassigned binding whose
/// initializer is a member/computed read). That is the VM's own
/// `defineProperty(frame.stack[...], operand, descriptor)` operation, which
/// writes a runtime value that came from a VM object. The readable
/// standard-built-ins assumption covers such VM-local writes, so they record no
/// barrier. The shape is not extended to parameters, call results, unresolved
/// globals, or reassigned locals: those can still be the global object, and a
/// source-visible `f(globalThis, ...)` call must keep the saved identity.
///
/// `Conservative` keeps the full barrier for every unproven target. A literal
/// native key on any unproven target keeps its binding barrier under both
/// policies because the source names the rebinding target.
#[allow(clippy::too_many_arguments)]
fn record_define_member_write(
    mutations: &mut MutationSet,
    target: &RawExpr,
    key: Option<&str>,
    values: &HashMap<SymbolId, Value>,
    raw: &HashMap<SymbolId, RawInit>,
    scoping: &Scoping,
    policy: NativeCleanupPolicy,
    span: Span,
) {
    if expression_is_global_object(target, values) {
        record_global_define_key(mutations, key);
        return;
    }
    if let Some(roots) = native_object_roots(target, values) {
        mutations.member_roots.extend(roots);
    } else {
        // A dynamic key on a VM-local member value is the only unproven write
        // that readable mode treats as a plain property write. Literal keys,
        // parameters, call results, and reassigned locals keep both barriers.
        if policy == NativeCleanupPolicy::Readable
            && key.is_none()
            && opaque_target_is_local_member_value(target, raw, scoping)
        {
            return;
        }
        // The target is not modeled: it could alias `globalThis` (or a native
        // prototype). Keep both barriers.
        record_global_define_key(mutations, key);
        mutations.unknown_member_write = true;
        if mutations.unknown_member_site.is_none() {
            mutations.unknown_member_site = Some(span);
        }
    }
}

/// True when an opaque `defineProperty` target is a non-reassigned local whose
/// initializer is a member or computed read (`var target = frame.stack[index]`).
///
/// This is a deliberately bounded readable-mode shape, not a general proof: a
/// member read can still return the global object at runtime. It exists so the
/// VM's internal `defineProperty(stackValue, operand, ...)` opcode does not
/// poison the captured native catalogue with a potential-global barrier. The
/// caller restricts it to dynamic keys; literal native keys keep their barrier.
fn opaque_target_is_local_member_value(
    target: &RawExpr,
    raw: &HashMap<SymbolId, RawInit>,
    scoping: &Scoping,
) -> bool {
    let mut current = target;
    let mut seen = HashSet::new();
    loop {
        match current {
            // A direct member/computed read is the same shape: a value fetched
            // from another object at runtime.
            RawExpr::Member(_, _) | RawExpr::Computed(_) => return true,
            RawExpr::Alias(symbol) => {
                if !seen.insert(*symbol) || scoping.symbol_is_mutated(*symbol) {
                    return false;
                }
                match raw.get(symbol) {
                    Some(RawInit::Expr(expression)) => current = expression,
                    _ => return false,
                }
            }
            _ => return false,
        }
    }
}

/// The set of native roots whose object identity an expression is provably
/// built from: a native global, a member of the global object, `X.prototype`
/// for a native-rooted `X`, or an alias that resolves to a non-empty native
/// root set. A member read whose pointee is not modelled returns `None`.
fn native_object_roots(
    expression: &RawExpr,
    values: &HashMap<SymbolId, Value>,
) -> Option<BTreeSet<String>> {
    match expression {
        RawExpr::Global(name) if analysis::is_native_global(name) => {
            Some([name.clone()].into_iter().collect())
        }
        RawExpr::Global(_) => None,
        RawExpr::Member(base, property) => {
            if let RawExpr::Global(root) = base.as_ref() {
                if matches!(root.as_str(), "globalThis" | "window" | "self")
                    && analysis::is_native_global(property)
                {
                    return Some([property.clone()].into_iter().collect());
                }
            }
            if property == "prototype" {
                if let RawExpr::Global(name) = base.as_ref() {
                    if analysis::is_native_global(name) {
                        return Some([name.clone()].into_iter().collect());
                    }
                }
                if let RawExpr::Alias(symbol) = base.as_ref() {
                    return values.get(symbol).and_then(native_roots_of_value);
                }
            }
            None
        }
        RawExpr::Alias(symbol) => values.get(symbol).and_then(native_roots_of_value),
        RawExpr::Null | RawExpr::EmptyArray | RawExpr::EmptyObject | RawExpr::EmptyString => None,
        RawExpr::Computed(_) | RawExpr::Unknown => None,
    }
}

fn native_roots_of_value(value: &Value) -> Option<BTreeSet<String>> {
    (value.kind != ValueKind::FreshLiteral && !value.roots.is_empty()).then(|| value.roots.clone())
}

/// Classify the first argument of a `defineProperty` call.
///
/// Only `Local` suppresses the whole-catalogue barrier:
///
/// * `Local` — the fresh literal object itself (`[]`, `{}`, `""`), `null`, or
///   `X.prototype` where `X` is a stable, never-reassigned function/class
///   declaration. A write to one of these cannot reach a native global.
/// * `Native` — a native global or a member of a known global object
///   (`globalThis.Array`); the write is real and the full barrier is kept.
/// * `Opaque` — everything not proven local: an unresolved global, a function
///   parameter or other binding with no recorded initializer, a call result, a
///   member read whose pointee is unmodeled (`box.native`, `[].constructor`), a
///   computed read (`frame.stack[0]`), or a function/class binding that is
///   reassigned. Base locality does not prove the pointee is local, so member
///   and computed reads never inherit the base's class.
#[allow(clippy::too_many_arguments)]
fn classify_define_target(
    expression: &RawExpr,
    raw: &HashMap<SymbolId, RawInit>,
    values: &HashMap<SymbolId, Value>,
    scoping: &Scoping,
    prototype_written: &HashSet<SymbolId>,
    visited: &mut HashSet<SymbolId>,
) -> TargetClass {
    match expression {
        RawExpr::EmptyArray | RawExpr::EmptyObject | RawExpr::EmptyString | RawExpr::Null => {
            TargetClass::Local
        }
        RawExpr::Global(name) => {
            if analysis::is_native_global(name) {
                TargetClass::Native
            } else {
                TargetClass::Opaque
            }
        }
        RawExpr::Member(base, property) => {
            if let RawExpr::Global(root) = base.as_ref() {
                if matches!(root.as_str(), "globalThis" | "window" | "self")
                    && analysis::is_native_global(property)
                {
                    return TargetClass::Native;
                }
            }
            // The one fresh-producing member read we can prove: `X.prototype`
            // for a stable fresh function/class binding is itself fresh.
            if property == "prototype" {
                if let RawExpr::Alias(symbol) = base.as_ref() {
                    if binding_is_fresh_function(*symbol, scoping, prototype_written) {
                        return TargetClass::Local;
                    }
                }
            }
            // Any other member read returns an object we do not model. The base
            // being local says nothing about the pointee (`box.native`), and
            // `[].constructor` is the native `Array` despite the fresh base.
            TargetClass::Opaque
        }
        RawExpr::Alias(symbol) => {
            classify_define_alias(*symbol, raw, values, scoping, prototype_written, visited)
        }
        // Opaque expressions include computed reads (`frame.stack[0]`), which
        // can hold any runtime value, including a native prototype.
        RawExpr::Computed(_) | RawExpr::Unknown => TargetClass::Opaque,
    }
}

/// True when `symbol` names a function or class binding that has not been
/// reassigned and whose `prototype` member has not been overwritten, so the
/// function object and its `.prototype` are fresh values.
fn binding_is_fresh_function(
    symbol: SymbolId,
    scoping: &Scoping,
    prototype_written: &HashSet<SymbolId>,
) -> bool {
    let flags = scoping.symbol_flags(symbol);
    (flags.is_function() || flags.is_class())
        && !scoping.symbol_is_mutated(symbol)
        && !prototype_written.contains(&symbol)
}

#[allow(clippy::too_many_arguments)]
fn classify_define_alias(
    symbol: SymbolId,
    raw: &HashMap<SymbolId, RawInit>,
    values: &HashMap<SymbolId, Value>,
    scoping: &Scoping,
    prototype_written: &HashSet<SymbolId>,
    visited: &mut HashSet<SymbolId>,
) -> TargetClass {
    if let Some(value) = values.get(&symbol) {
        return if value.kind == ValueKind::FreshLiteral || value.roots.is_empty() {
            TargetClass::Local
        } else {
            TargetClass::Native
        };
    }
    if !visited.insert(symbol) {
        return TargetClass::Opaque;
    }
    match raw.get(&symbol) {
        // Not a collected variable declarator. A function/class declaration is
        // a fresh object unless it is reassigned; a parameter, catch binding,
        // import, or destructured binding is an unknown value and stays opaque.
        None => {
            if binding_is_fresh_function(symbol, scoping, prototype_written) {
                TargetClass::Local
            } else {
                TargetClass::Opaque
            }
        }
        Some(RawInit::Expr(expression)) => {
            classify_define_target(expression, raw, values, scoping, prototype_written, visited)
        }
        Some(RawInit::Conditional {
            consequent,
            alternate,
            ..
        }) => {
            let consequent = classify_define_target(
                consequent,
                raw,
                values,
                scoping,
                prototype_written,
                visited,
            );
            let alternate =
                classify_define_target(alternate, raw, values, scoping, prototype_written, visited);
            if consequent == TargetClass::Local && alternate == TargetClass::Local {
                TargetClass::Local
            } else {
                TargetClass::Opaque
            }
        }
        Some(RawInit::FactoryCall { .. } | RawInit::ApplyFactory { .. } | RawInit::Unknown) => {
            TargetClass::Opaque
        }
    }
}

/// Record a written member path.
///
/// A path that names exactly a native global (`Array = other`) or a global
/// object member (`globalThis.Array = other`) reassigns that binding, so it
/// invalidates every alias rooted there, constructors included. A deeper path
/// (`Array.prototype.push = evil`) is a *property write*: it is recorded as a
/// member barrier, but conservative mode also keeps the whole-root barrier
/// because a plain assignment is not the target-classified `defineProperty`
/// call. A target-classified `defineProperty` write records only the member
/// barrier through [`record_define_member_write`].
fn record_mutation_path(mutations: &mut MutationSet, path: &str) {
    let mut parts = path.split('.');
    let Some(root) = parts.next() else {
        return;
    };
    let (native_root, rest) = if matches!(root, "globalThis" | "window" | "self") {
        let Some(native_root) = parts.next() else {
            // A write to (or delete of) the whole global object can reach any
            // global binding.
            record_all_native_mutation(mutations);
            return;
        };
        if !analysis::is_native_global(native_root) {
            return;
        }
        (native_root, parts.collect::<Vec<_>>())
    } else if analysis::is_native_global(root) {
        (root, parts.collect::<Vec<_>>())
    } else {
        return;
    };
    if rest.is_empty() {
        mutations.roots.insert(native_root.into());
        mutations.binding_roots.insert(native_root.into());
    } else if rest.contains(&"[]") {
        mutations.roots.insert(native_root.into());
        mutations.member_roots.insert(native_root.into());
        mutations.unknown_member_write = true;
    } else {
        let canonical = std::iter::once(native_root)
            .chain(rest.iter().copied())
            .collect::<Vec<_>>()
            .join(".");
        mutations.roots.insert(native_root.into());
        mutations.member_roots.insert(native_root.into());
        mutations.paths.insert(canonical);
    }
}

fn record_all_native_mutation(mutations: &mut MutationSet) {
    // Reaching the whole global object can replace any global binding, so this
    // is a binding barrier and not merely a property write: readable mode must
    // retain saved constructor identities too.
    mutations
        .roots
        .extend(analysis::NATIVE_GLOBALS.iter().map(|name| (*name).into()));
    mutations
        .binding_roots
        .extend(analysis::NATIVE_GLOBALS.iter().map(|name| (*name).into()));
}

fn reference_captures_native(
    scoping: &Scoping,
    value: &Value,
    reference: &oxc_semantic::Reference,
    mutations: &MutationSet,
) -> bool {
    value.roots.iter().any(|root| {
        let Some(binding) = scoping.find_binding(reference.scope_id(), Ident::from(root.as_str()))
        else {
            return false;
        };
        // Oxc models an assignment to an unresolved global member as a
        // mutable global binding. That synthetic binding is not lexical
        // shadowing; under the readable property-stability assumption it
        // must not block a captured native method.
        !(mutations.member_roots.contains(root) && scoping.symbol_is_mutated(binding))
    })
}

fn factory_calls_cover_symbol(
    symbol: SymbolId,
    calls: &[Replacement],
    ignored_declarations: &[Span],
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
) -> bool {
    let mut found = false;
    for reference in scoping.get_resolved_references(symbol) {
        if !reference.flags().contains(ReferenceFlags::Read) {
            continue;
        }
        if let AstKind::IdentifierReference(identifier) = nodes.kind(reference.node_id()) {
            if calls
                .iter()
                .any(|call| contains(call.span, identifier.span))
                || ignored_declarations
                    .iter()
                    .any(|declaration| contains(*declaration, identifier.span))
            {
                found = true;
            } else {
                return false;
            }
        }
    }
    found
}

fn protected_uncurry_dependencies(
    candidates: &[Candidate],
    raw: &HashMap<SymbolId, RawInit>,
    values: &HashMap<SymbolId, Value>,
    factory_calls: &[Replacement],
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    policy: NativeCleanupPolicy,
) -> HashSet<SymbolId> {
    let mut protected = candidates
        .iter()
        .filter(|candidate| {
            values.get(&candidate.symbol).is_some_and(|value| {
                policy == NativeCleanupPolicy::Conservative
                    && matches!(value.kind, ValueKind::Uncurried | ValueKind::GenericApply)
                    && has_non_callee_read(candidate.symbol, scoping, nodes)
            })
        })
        .map(|candidate| candidate.symbol)
        .collect::<HashSet<_>>();

    loop {
        let before = protected.len();
        let mut pending = protected.iter().copied().collect::<Vec<_>>();
        while let Some(symbol) = pending.pop() {
            for dependency in raw_dependencies(raw.get(&symbol)) {
                if protected.insert(dependency) {
                    pending.push(dependency);
                }
            }
        }

        let ignored_declarations = candidates
            .iter()
            .filter(|candidate| {
                values.get(&candidate.symbol).is_some_and(|value| {
                    matches!(value.kind, ValueKind::Uncurried | ValueKind::GenericApply)
                        && !protected.contains(&candidate.symbol)
                })
            })
            .map(|candidate| candidate.declaration.span)
            .collect::<Vec<_>>();
        for candidate in candidates {
            if policy == NativeCleanupPolicy::Readable
                || !matches!(
                    values.get(&candidate.symbol).map(|value| value.kind),
                    Some(ValueKind::Factory)
                )
                || !has_read(candidate.symbol, scoping)
                || factory_calls_cover_symbol(
                    candidate.symbol,
                    factory_calls,
                    &ignored_declarations,
                    scoping,
                    nodes,
                )
            {
                continue;
            }
            if protected.insert(candidate.symbol) {
                pending.push(candidate.symbol);
            }
        }
        while let Some(symbol) = pending.pop() {
            for dependency in raw_dependencies(raw.get(&symbol)) {
                if protected.insert(dependency) {
                    pending.push(dependency);
                }
            }
        }
        if protected.len() == before {
            break;
        }
    }

    // Readable mode still has to preserve first-class bound-function identity:
    // `var callBound = uncurryThis(nativePush); use(callBound);` captures one
    // bound function, while rewriting the use to
    // `Function.prototype.call.bind(Array.prototype.push)` allocates a fresh
    // function on every evaluation. Retain such an adapter (and its captured
    // dependency chain) unless every read is a direct callee or lives in a
    // derived declaration that is itself being inlined away.
    if policy == NativeCleanupPolicy::Readable {
        loop {
            let before = protected.len();
            let ignored_declarations = candidates
                .iter()
                .filter(|candidate| {
                    values.get(&candidate.symbol).is_some_and(|value| {
                        matches!(value.kind, ValueKind::Uncurried | ValueKind::GenericApply)
                            && !protected.contains(&candidate.symbol)
                    })
                })
                .map(|candidate| candidate.declarator_span)
                .collect::<Vec<_>>();
            for candidate in candidates {
                if protected.contains(&candidate.symbol) || !has_read(candidate.symbol, scoping) {
                    continue;
                }
                let bound = values.get(&candidate.symbol).is_some_and(|value| {
                    matches!(
                        value.kind,
                        ValueKind::Uncurried | ValueKind::GenericApply | ValueKind::Factory
                    )
                });
                if bound
                    && has_non_callee_read_outside(
                        candidate.symbol,
                        &ignored_declarations,
                        scoping,
                        nodes,
                    )
                {
                    protected.insert(candidate.symbol);
                }
            }
            let mut pending = protected.iter().copied().collect::<Vec<_>>();
            while let Some(symbol) = pending.pop() {
                for dependency in raw_dependencies(raw.get(&symbol)) {
                    if protected.insert(dependency) {
                        pending.push(dependency);
                    }
                }
            }
            if protected.len() == before {
                break;
            }
        }
    }
    protected
}

fn has_read(symbol: SymbolId, scoping: &Scoping) -> bool {
    scoping
        .get_resolved_references(symbol)
        .any(|reference| reference.flags().contains(ReferenceFlags::Read))
}

fn has_non_callee_read(symbol: SymbolId, scoping: &Scoping, nodes: &AstNodes<'_>) -> bool {
    scoping
        .get_resolved_references(symbol)
        .filter(|reference| reference.flags().contains(ReferenceFlags::Read))
        .any(|reference| {
            let AstKind::IdentifierReference(identifier) = nodes.kind(reference.node_id()) else {
                return true;
            };
            !matches!(
                nodes.parent_kind(reference.node_id()),
                AstKind::CallExpression(call) if contains(call.callee.span(), identifier.span)
            )
        })
}

/// A read that is not a direct callee and is not inside a derived declaration
/// that the pass is about to inline away.
fn has_non_callee_read_outside(
    symbol: SymbolId,
    ignored_declarations: &[Span],
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
) -> bool {
    scoping
        .get_resolved_references(symbol)
        .filter(|reference| reference.flags().contains(ReferenceFlags::Read))
        .any(|reference| {
            let AstKind::IdentifierReference(identifier) = nodes.kind(reference.node_id()) else {
                return true;
            };
            if ignored_declarations
                .iter()
                .any(|declaration| contains(*declaration, identifier.span))
            {
                return false;
            }
            !matches!(
                nodes.parent_kind(reference.node_id()),
                AstKind::CallExpression(call) if contains(call.callee.span(), identifier.span)
            )
        })
}

fn raw_dependencies(raw: Option<&RawInit>) -> Vec<SymbolId> {
    let mut dependencies = Vec::new();
    let Some(raw) = raw else {
        return dependencies;
    };
    match raw {
        RawInit::Expr(expression) => raw_expr_dependencies(expression, &mut dependencies),
        RawInit::Conditional {
            consequent,
            alternate,
            ..
        } => {
            raw_expr_dependencies(consequent, &mut dependencies);
            raw_expr_dependencies(alternate, &mut dependencies);
        }
        RawInit::FactoryCall {
            call_base,
            bind_target,
            bound_this,
        } => dependencies.extend([*call_base, *bind_target, *bound_this]),
        RawInit::ApplyFactory { factory, argument } => {
            dependencies.push(*factory);
            raw_expr_dependencies(argument, &mut dependencies);
        }
        RawInit::Unknown => {}
    }
    dependencies
}

fn raw_expr_dependencies(expression: &RawExpr, dependencies: &mut Vec<SymbolId>) {
    match expression {
        RawExpr::Alias(symbol) => dependencies.push(*symbol),
        RawExpr::Member(base, _) | RawExpr::Computed(base) => {
            raw_expr_dependencies(base, dependencies);
        }
        _ => {}
    }
}

fn collect_factory_calls(
    program: &Program<'_>,
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    values: &HashMap<SymbolId, Value>,
) -> Vec<Replacement> {
    let mut visitor = FactoryCallVisitor {
        scoping,
        nodes,
        values,
        replacements: Vec::new(),
    };
    visitor.visit_program(program);
    visitor.replacements
}

struct FactoryCallVisitor<'a> {
    scoping: &'a Scoping,
    nodes: &'a AstNodes<'a>,
    values: &'a HashMap<SymbolId, Value>,
    replacements: Vec<Replacement>,
}

impl<'a> Visit<'a> for FactoryCallVisitor<'_> {
    fn visit_call_expression(&mut self, expression: &CallExpression<'a>) {
        if let Expression::Identifier(identifier) = &expression.callee {
            if let Some(factory) = analysis::identifier_symbol(identifier, self.scoping)
                .and_then(|symbol| self.values.get(&symbol))
            {
                if factory.kind == ValueKind::Factory {
                    let argument_value = match expression.arguments.first() {
                        Some(Argument::Identifier(argument)) => {
                            analysis::identifier_symbol(argument, self.scoping)
                                .and_then(|symbol| self.values.get(&symbol).cloned())
                        }
                        Some(argument) => argument.as_expression().and_then(|expression| {
                            let raw_argument = raw_expr(expression, self.scoping);
                            // Every catalogue candidate was resolved into
                            // `values` once, so this read-only lookup returns
                            // the same value as a mutable resolve without
                            // cloning the whole table per call site.
                            resolve_raw_frozen(&raw_argument, self.values)
                        }),
                        None => None,
                    };
                    if let Some(value) = argument_value {
                        if matches!(
                            value.kind,
                            ValueKind::PrototypeMethod | ValueKind::GenericApply
                        ) {
                            let directly_called = self
                                .nodes
                                .iter_enumerated()
                                .find_map(|(node_id, node)| {
                                    (node.span() == expression.span).then_some(node_id)
                                })
                                .is_some_and(|node_id| {
                                    matches!(
                                        self.nodes.parent_kind(node_id),
                                        AstKind::CallExpression(parent)
                                            if contains(parent.callee.span(), expression.span)
                                    )
                                });
                            if directly_called {
                                self.replacements.push(Replacement {
                                    span: expression.span,
                                    text: if value.kind == ValueKind::GenericApply {
                                        "Function.prototype.apply.call".into()
                                    } else {
                                        format!("{}.call", value.replacement)
                                    },
                                });
                            }
                        }
                    }
                }
            }
        }
        walk::walk_call_expression(self, expression);
    }
}

#[derive(Default)]
struct TableCleanup {
    edits: Vec<analysis::Edit>,
    declarations: Vec<Span>,
}

#[allow(clippy::too_many_arguments)]
fn clean_native_tables(
    _source: &str,
    program: &Program<'_>,
    tables: &[TableCandidate],
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    raw: &HashMap<SymbolId, RawInit>,
    values: &HashMap<SymbolId, Value>,
    mutations: &MutationSet,
    operation_function_spans: &HashMap<SymbolId, Vec<Span>>,
    report: &mut NativeCleanupReport,
    diagnostics: &mut Vec<Diagnostic>,
) -> TableCleanup {
    let mut result = TableCleanup::default();
    for table in tables {
        let resolved: Vec<_> = table
            .elements
            .iter()
            .map(|element| {
                element
                    .as_ref()
                    .and_then(|expression| resolve_table_expr(expression, values))
            })
            .collect();
        if resolved.iter().all(Option::is_none) {
            continue;
        }
        let table_mutated = value_is_table_mutated(table.symbol, scoping, mutations, raw);
        let mut visitor = TableUseVisitor {
            symbol: table.symbol,
            resolved: &resolved,
            scoping,
            replacements: Vec::new(),
            invalid_use: false,
        };
        if !table_mutated {
            visitor.visit_program(program);
        } else {
            visitor.invalid_use = true;
        }
        let all_refs_replaced = !visitor.invalid_use
            && scoping
                .get_resolved_references(table.symbol)
                .filter(|reference| reference.flags().contains(ReferenceFlags::Read))
                .all(|reference| {
                    matches!(nodes.kind(reference.node_id()), AstKind::IdentifierReference(id)
                        if is_fixed_table_reference(id.span, table.symbol, &resolved, scoping, nodes))
                });
        result
            .edits
            .extend(visitor.replacements.into_iter().map(|replacement| {
                report.object_indexed_refs += 1;
                analysis::Edit::replace(replacement.span, replacement.text)
            }));
        let unresolved_use_in_operation = scoping
            .get_resolved_references(table.symbol)
            .filter(|reference| reference.flags().contains(ReferenceFlags::Read))
            .filter_map(|reference| match nodes.kind(reference.node_id()) {
                AstKind::IdentifierReference(identifier) => Some(identifier.span),
                _ => None,
            })
            .any(|span| {
                !is_fixed_table_reference(span, table.symbol, &resolved, scoping, nodes)
                    && operation_function_spans
                        .values()
                        .flatten()
                        .any(|operation_span| contains(*operation_span, span))
            });
        if all_refs_replaced && !table_mutated {
            result.declarations.push(table.declaration.span);
            diagnostics.push(Diagnostic::info(
                Stage::Clean,
                "NATIVE_TABLE_INLINED",
                format!(
                    "expanded native catalogue `{}` and removed its declaration",
                    table.name
                ),
            ));
        } else {
            if unresolved_use_in_operation {
                if let Some(scope) = (!table.name.eq("vmNativeFunctions"))
                    .then(|| scoping.symbol_scope_id(table.symbol))
                {
                    if scoping
                        .get_binding(scope, Ident::from("vmNativeFunctions"))
                        .is_none()
                    {
                        let declaration_span = scoping.symbol_span(table.symbol);
                        let unresolved_spans = scoping
                            .get_resolved_references(table.symbol)
                            .filter(|reference| reference.flags().contains(ReferenceFlags::Read))
                            .filter_map(|reference| match nodes.kind(reference.node_id()) {
                                AstKind::IdentifierReference(identifier)
                                    if !is_fixed_table_reference(
                                        identifier.span,
                                        table.symbol,
                                        &resolved,
                                        scoping,
                                        nodes,
                                    ) =>
                                {
                                    Some(identifier.span)
                                }
                                _ => None,
                            });
                        for span in std::iter::once(declaration_span).chain(unresolved_spans) {
                            result
                                .edits
                                .push(analysis::Edit::replace(span, "vmNativeFunctions"));
                        }
                        diagnostics.push(Diagnostic::info(
                            Stage::Clean,
                            "NATIVE_TABLE_LABEL",
                            format!(
                                "retained native catalogue `{}` as `vmNativeFunctions`",
                                table.name
                            ),
                        ));
                    }
                }
            }
            diagnostics.push(Diagnostic::warn(
                Stage::Clean,
                "NATIVE_TABLE_RETAINED",
                format!(
                    "retained native catalogue `{}` for dynamic, hole, out-of-bounds, or reflective use",
                    table.name
                ),
            ));
        }
    }
    result
}

fn value_is_table_mutated(
    symbol: SymbolId,
    scoping: &Scoping,
    mutations: &MutationSet,
    raw: &HashMap<SymbolId, RawInit>,
) -> bool {
    if scoping.symbol_is_mutated(symbol) || mutations.symbols.contains(&symbol) {
        return true;
    }
    raw.iter().any(|(candidate, initializer)| {
        matches!(initializer, RawInit::Expr(RawExpr::Alias(target)) if {
            *target == symbol || raw_alias_reaches(*target, symbol, raw, &mut HashSet::new())
        }) && mutations.symbols.contains(candidate)
    })
}

fn raw_alias_reaches(
    symbol: SymbolId,
    target: SymbolId,
    raw: &HashMap<SymbolId, RawInit>,
    visited: &mut HashSet<SymbolId>,
) -> bool {
    if symbol == target || !visited.insert(symbol) {
        return symbol == target;
    }
    match raw.get(&symbol) {
        Some(RawInit::Expr(RawExpr::Alias(next))) => raw_alias_reaches(*next, target, raw, visited),
        _ => false,
    }
}

fn resolve_table_expr(expression: &RawExpr, values: &HashMap<SymbolId, Value>) -> Option<Value> {
    let value = match expression {
        RawExpr::Alias(symbol) => values.get(symbol).cloned(),
        RawExpr::Global(name) if analysis::is_native_global(name) => Some(Value {
            replacement: name.clone(),
            roots: [name.clone()].into_iter().collect(),
            kind: ValueKind::Plain,
        }),
        RawExpr::Global(_) => None,
        _ => match expression {
            RawExpr::Member(_, _) => resolve_raw_frozen(expression, values),
            _ => None,
        },
    };
    // A catalogue element that is a fresh literal (directly, or through an
    // alias) is an identity. Expanding `OO[0]` to `Array`/`Object`/`String`
    // would fabricate a shared global, so decline to expand it.
    value.filter(|value| value.kind != ValueKind::FreshLiteral)
}

struct TableUseVisitor<'a> {
    symbol: SymbolId,
    resolved: &'a [Option<Value>],
    scoping: &'a Scoping,
    replacements: Vec<Replacement>,
    invalid_use: bool,
}

fn is_fixed_table_reference(
    span: Span,
    symbol: SymbolId,
    resolved: &[Option<Value>],
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
) -> bool {
    scoping
        .get_resolved_references(symbol)
        .filter(|reference| reference.flags().contains(ReferenceFlags::Read))
        .find_map(|reference| match nodes.kind(reference.node_id()) {
            AstKind::IdentifierReference(identifier) if identifier.span == span => {
                match nodes.parent_kind(reference.node_id()) {
                    AstKind::ComputedMemberExpression(member)
                        if contains(member.object.span(), identifier.span) =>
                    {
                        let Expression::NumericLiteral(number) = &member.expression else {
                            return Some(false);
                        };
                        if !number.value.is_finite()
                            || number.value < 0.0
                            || number.value.fract() != 0.0
                        {
                            return Some(false);
                        }
                        Some(
                            resolved
                                .get(number.value as usize)
                                .is_some_and(Option::is_some),
                        )
                    }
                    _ => Some(false),
                }
            }
            _ => None,
        })
        .unwrap_or(false)
}

impl<'a> Visit<'a> for TableUseVisitor<'_> {
    fn visit_computed_member_expression(&mut self, member: &ComputedMemberExpression<'a>) {
        let base_symbol = match &member.object {
            Expression::Identifier(identifier) => {
                analysis::identifier_symbol(identifier, self.scoping)
            }
            _ => None,
        };
        if base_symbol == Some(self.symbol) {
            let index = match &member.expression {
                Expression::NumericLiteral(number)
                    if number.value.is_finite()
                        && number.value >= 0.0
                        && number.value.fract() == 0.0 =>
                {
                    Some(number.value as usize)
                }
                _ => None,
            };
            match index.and_then(|index| self.resolved.get(index).and_then(Clone::clone)) {
                Some(value) => self.replacements.push(Replacement {
                    span: member.span,
                    text: replacement_text(&value),
                }),
                None => self.invalid_use = true,
            }
        }
        walk::walk_computed_member_expression(self, member);
    }
}

impl<'a> Visit<'a> for Collector<'_> {
    fn visit_with_statement(&mut self, _: &WithStatement<'a>) {
        self.dynamic = true;
    }

    fn visit_call_expression(&mut self, expression: &CallExpression<'a>) {
        if let Expression::Identifier(identifier) = &expression.callee {
            if identifier.name == "eval" && identifier.reference_id.get().is_none() {
                self.dynamic = true;
            }
        }
        walk::walk_call_expression(self, expression);
    }

    fn visit_variable_declaration(&mut self, declaration: &VariableDeclaration<'a>) {
        let previous = self.current.take();
        self.current = Some(DeclarationInfo {
            span: declaration.span,
            declarators: declaration.declarations.iter().map(GetSpan::span).collect(),
        });
        walk::walk_variable_declaration(self, declaration);
        self.current = previous;
    }

    fn visit_variable_declarator(&mut self, declarator: &VariableDeclarator<'a>) {
        let Some(declaration) = self.current.clone() else {
            walk::walk_variable_declarator(self, declarator);
            return;
        };
        let Some(binding) = simple_binding(&declarator.id) else {
            walk::walk_variable_declarator(self, declarator);
            return;
        };
        let Some(symbol) = binding.symbol_id.get() else {
            walk::walk_variable_declarator(self, declarator);
            return;
        };
        let raw = declarator
            .init
            .as_ref()
            .map(|expression| raw_init(expression, self.scoping, self.source))
            .unwrap_or(RawInit::Unknown);
        if let Some(init) = declarator.init.as_ref() {
            if let Some(elements) = native_table_elements(init, self.scoping) {
                self.tables.push(TableCandidate {
                    symbol,
                    name: binding.name.to_string(),
                    declaration: declaration.clone(),
                    elements,
                });
            }
        }
        self.candidates.push(Candidate {
            symbol,
            name: binding.name.to_string(),
            declaration,
            declarator_span: declarator.span,
            raw,
        });
        walk::walk_variable_declarator(self, declarator);
    }
}

fn raw_init(expression: &Expression<'_>, scoping: &Scoping, source: &str) -> RawInit {
    if let Expression::ConditionalExpression(conditional) = expression {
        let alternate_is_native = matches!(&conditional.alternate, Expression::NullLiteral(_))
            || raw_expr_is_native(&raw_expr(&conditional.alternate, scoping));
        let consequent_is_native = raw_expr_is_native(&raw_expr(&conditional.consequent, scoping));
        if is_typeof_function_test(&conditional.test) && alternate_is_native && consequent_is_native
        {
            return RawInit::Conditional {
                test: source
                    [conditional.test.span().start as usize..conditional.test.span().end as usize]
                    .into(),
                consequent: raw_expr(&conditional.consequent, scoping),
                alternate: raw_expr(&conditional.alternate, scoping),
                roots: conditional_roots(
                    &conditional.test,
                    &conditional.consequent,
                    &conditional.alternate,
                ),
            };
        }
    }
    if let Expression::CallExpression(call) = expression {
        if let Some((call_base, bind_target, bound_this)) = factory_call_shape(call, scoping) {
            return RawInit::FactoryCall {
                call_base,
                bind_target,
                bound_this,
            };
        }
        if let Some((factory, argument)) = apply_factory_shape(call, scoping) {
            return RawInit::ApplyFactory { factory, argument };
        }
    }
    // `Object["defineProperty"]` with a literal key is the modelled mutation
    // member, not an unknown value. Resolve identifier aliases of this form so
    // they carry the same target/key barrier as `D = Object.defineProperty`.
    if let Expression::ComputedMemberExpression(member) = expression {
        if matches!(&member.expression, Expression::StringLiteral(key) if key.value == "defineProperty")
        {
            return RawInit::Expr(RawExpr::Member(
                Box::new(raw_expr(&member.object, scoping)),
                "defineProperty".into(),
            ));
        }
    }
    RawInit::Expr(raw_expr(expression, scoping))
}

fn raw_expr_is_native(expression: &RawExpr) -> bool {
    match expression {
        RawExpr::Global(name) => analysis::is_native_global(name),
        RawExpr::Member(base, _) => raw_expr_is_native(base),
        // The alias graph is resolved after collection.  Treating an alias as
        // a possible native branch here lets a later semantic resolution
        // decide whether it really reaches a native value.
        RawExpr::Alias(_) | RawExpr::Null => true,
        _ => false,
    }
}

fn raw_expr(expression: &Expression<'_>, scoping: &Scoping) -> RawExpr {
    match expression {
        Expression::Identifier(identifier) => analysis::identifier_symbol(identifier, scoping)
            .map_or_else(
                || RawExpr::Global(identifier.name.to_string()),
                RawExpr::Alias,
            ),
        Expression::StaticMemberExpression(member) => RawExpr::Member(
            Box::new(raw_expr(&member.object, scoping)),
            member.property.name.to_string(),
        ),
        // A computed member read is still a value fetched from another object
        // at runtime. It stays opaque for value resolution/property purity but
        // is distinguishable from a call result or an arbitrary expression for
        // the readable defineProperty-target shape check.
        Expression::ComputedMemberExpression(member) => {
            RawExpr::Computed(Box::new(raw_expr(&member.object, scoping)))
        }
        Expression::NullLiteral(_) => RawExpr::Null,
        Expression::ArrayExpression(array) if array.elements.is_empty() => RawExpr::EmptyArray,
        Expression::ObjectExpression(object) if object.properties.is_empty() => {
            RawExpr::EmptyObject
        }
        Expression::StringLiteral(string) if string.value.is_empty() => RawExpr::EmptyString,
        _ => RawExpr::Unknown,
    }
}

fn is_typeof_function_test(expression: &Expression<'_>) -> bool {
    matches!(expression, Expression::BinaryExpression(binary) if binary.operator == BinaryOperator::StrictEquality && matches!(&binary.left, Expression::UnaryExpression(unary) if unary.operator == UnaryOperator::Typeof) && matches!(&binary.right, Expression::StringLiteral(string) if string.value == "function")
        || matches!(expression, Expression::BinaryExpression(binary) if binary.operator == BinaryOperator::StrictEquality && matches!(&binary.right, Expression::UnaryExpression(unary) if unary.operator == UnaryOperator::Typeof) && matches!(&binary.left, Expression::StringLiteral(string) if string.value == "function")))
}

fn conditional_roots(
    test: &Expression<'_>,
    consequent: &Expression<'_>,
    alternate: &Expression<'_>,
) -> BTreeSet<String> {
    let mut roots = BTreeSet::new();
    if let Expression::BinaryExpression(binary) = test {
        let candidate = match (&binary.left, &binary.right) {
            (Expression::UnaryExpression(unary), _) if unary.operator == UnaryOperator::Typeof => {
                identifier_name(&unary.argument)
            }
            (_, Expression::UnaryExpression(unary)) if unary.operator == UnaryOperator::Typeof => {
                identifier_name(&unary.argument)
            }
            _ => None,
        };
        if let Some(name) = candidate {
            roots.insert(name);
        }
    }
    collect_native_roots(consequent, &mut roots);
    collect_native_roots(alternate, &mut roots);
    roots
}

fn collect_native_roots(expression: &Expression<'_>, roots: &mut BTreeSet<String>) {
    match expression {
        Expression::Identifier(identifier)
            if analysis::is_native_global(identifier.name.as_str()) =>
        {
            roots.insert(identifier.name.to_string());
        }
        Expression::StaticMemberExpression(member) => collect_native_roots(&member.object, roots),
        Expression::ComputedMemberExpression(member) => collect_native_roots(&member.object, roots),
        _ => {}
    }
}

fn identifier_name(expression: &Expression<'_>) -> Option<String> {
    matches!(expression, Expression::Identifier(_)).then(|| match expression {
        Expression::Identifier(identifier) => identifier.name.to_string(),
        _ => unreachable!(),
    })
}

fn factory_call_shape(
    call: &CallExpression<'_>,
    scoping: &Scoping,
) -> Option<(SymbolId, SymbolId, SymbolId)> {
    let Expression::StaticMemberExpression(member) = &call.callee else {
        return None;
    };
    if member.property.name != "bind" || call.arguments.len() != 2 {
        return None;
    }
    let Expression::Identifier(base) = &member.object else {
        return None;
    };
    let Argument::Identifier(target) = &call.arguments[0] else {
        return None;
    };
    let Argument::Identifier(this_value) = &call.arguments[1] else {
        return None;
    };
    Some((
        analysis::identifier_symbol(base, scoping)?,
        analysis::identifier_symbol(target, scoping)?,
        analysis::identifier_symbol(this_value, scoping)?,
    ))
}

fn apply_factory_shape(
    call: &CallExpression<'_>,
    scoping: &Scoping,
) -> Option<(SymbolId, RawExpr)> {
    let Expression::Identifier(factory) = &call.callee else {
        return None;
    };
    let argument = call.arguments.first()?.as_expression()?;
    Some((
        analysis::identifier_symbol(factory, scoping)?,
        raw_expr(argument, scoping),
    ))
}

fn native_table_elements(
    expression: &Expression<'_>,
    scoping: &Scoping,
) -> Option<Vec<Option<RawExpr>>> {
    let Expression::ArrayExpression(array) = expression else {
        return None;
    };
    (!array.elements.is_empty()).then(|| {
        array
            .elements
            .iter()
            .map(|element| {
                element
                    .as_expression()
                    .map(|expression| raw_expr(expression, scoping))
            })
            .collect()
    })
}

fn function_array_spans(expression: &Expression<'_>) -> Option<Vec<Span>> {
    let Expression::ArrayExpression(array) = expression else {
        return None;
    };
    let spans: Option<Vec<_>> = array
        .elements
        .iter()
        .map(|element| {
            let expression = element.as_expression()?;
            matches!(
                expression,
                Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_)
            )
            .then_some(expression.span())
        })
        .collect();
    spans.filter(|spans| spans.len() >= 4)
}

fn proven_operation_function_spans(
    program: &Program<'_>,
    scoping: &Scoping,
) -> HashMap<SymbolId, Vec<Span>> {
    let mut collector = OperationTableCollector {
        scoping,
        function_tables: HashMap::new(),
        called_tables: HashSet::new(),
    };
    collector.visit_program(program);
    collector
        .function_tables
        .into_iter()
        .filter(|(symbol, _)| collector.called_tables.contains(symbol))
        .collect()
}

impl<'ast> Visit<'ast> for OperationTableCollector<'_> {
    fn visit_variable_declarator(&mut self, declarator: &VariableDeclarator<'ast>) {
        if let Some(binding) = simple_binding(&declarator.id) {
            if let (Some(symbol), Some(init)) = (binding.symbol_id.get(), declarator.init.as_ref())
            {
                if let Some(spans) = function_array_spans(init) {
                    self.function_tables.insert(symbol, spans);
                }
            }
        }
        walk::walk_variable_declarator(self, declarator);
    }

    fn visit_call_expression(&mut self, expression: &CallExpression<'ast>) {
        if let Expression::ComputedMemberExpression(member) = &expression.callee {
            if let Expression::Identifier(identifier) = &member.object {
                if let Some(symbol) = analysis::identifier_symbol(identifier, self.scoping) {
                    self.called_tables.insert(symbol);
                }
            }
        }
        walk::walk_call_expression(self, expression);
    }
}

fn simple_binding<'a>(pattern: &'a BindingPattern<'a>) -> Option<&'a BindingIdentifier<'a>> {
    match pattern {
        BindingPattern::BindingIdentifier(id) => Some(id),
        _ => None,
    }
}

fn has_dynamic_scope(program: &Program<'_>, scoping: &Scoping) -> bool {
    let mut visitor = DynamicVisitor {
        dynamic: false,
        scoping,
    };
    visitor.visit_program(program);
    visitor.dynamic
}

struct DynamicVisitor<'a> {
    dynamic: bool,
    scoping: &'a Scoping,
}
impl<'a> Visit<'a> for DynamicVisitor<'_> {
    fn visit_with_statement(&mut self, _: &WithStatement<'a>) {
        self.dynamic = true;
    }

    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        // Any reference to the *global* `eval` is a dynamic-scope hazard.
        // Direct `eval(...)` can see and reassign the enclosing scope; an
        // indirect `(0, eval)(...)` or aliased `var e = eval` runs in global
        // scope and can reassign top-level `var` bindings and native globals.
        // Treating the binding as no longer provable is the conservative choice.
        if identifier.name == "eval"
            && identifier
                .reference_id
                .get()
                .is_none_or(|reference| self.scoping.get_reference(reference).symbol_id().is_none())
        {
            self.dynamic = true;
        }
        walk::walk_identifier_reference(self, identifier);
    }

    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        walk::walk_call_expression(self, call);
    }
}

fn reparse_codegen(source: &str) -> Result<String, String> {
    let allocator = Allocator::default();
    let parsed = analysis::parse_program(&allocator, source)?;
    Ok(analysis::codegen(&parsed))
}

impl<'a> Visit<'a> for MutationCollector<'_> {
    fn visit_with_statement(&mut self, _: &WithStatement<'a>) {
        self.dynamic = true;
    }

    fn visit_assignment_expression(&mut self, expression: &AssignmentExpression<'a>) {
        if let Some(symbol) = assignment_target_symbol(&expression.left, self.scoping) {
            self.mutations.symbols.insert(symbol);
        }
        if let Some(path) = assignment_target_path(&expression.left) {
            self.record(path);
        }
        self.record_assignment_target(&expression.left, expression.span);
        walk::walk_assignment_expression(self, expression);
    }

    fn visit_update_expression(&mut self, expression: &UpdateExpression<'a>) {
        if let Some(symbol) = simple_target_symbol(&expression.argument, self.scoping) {
            self.mutations.symbols.insert(symbol);
        }
        if let Some(path) = simple_target_path(&expression.argument) {
            self.record(path);
        }
        walk::walk_update_expression(self, expression);
    }

    fn visit_unary_expression(&mut self, expression: &UnaryExpression<'a>) {
        if expression.operator == UnaryOperator::Delete {
            if let Some(path) = expression_path(&expression.argument) {
                self.record(path);
            }
        }
        walk::walk_unary_expression(self, expression);
    }

    fn visit_call_expression(&mut self, expression: &CallExpression<'a>) {
        if is_mutating_native_call(&expression.callee) {
            self.mutations
                .roots
                .extend(analysis::NATIVE_GLOBALS.iter().map(|name| (*name).into()));
            if expression
                .arguments
                .first()
                .and_then(Argument::as_expression)
                .is_some_and(|target| opaque_property_target(target, self.scoping))
            {
                self.mutations.unknown_member_write = true;
            }
        }
        if let Expression::StaticMemberExpression(member) = &expression.callee {
            if matches!(
                member.property.name.as_str(),
                "__defineGetter__" | "__defineSetter__" | "defineProperty"
            ) {
                let is_define_property = member.property.name.as_str() == "defineProperty";
                let define_target =
                    is_define_property.then(|| define_target_raw(expression, self.scoping));
                let write_key = is_define_property
                    .then(|| define_key_raw(expression))
                    .flatten();
                self.record_member_target(
                    &member.object,
                    Some(member.property.name.to_string()),
                    define_target,
                    write_key,
                    expression.span,
                );
            }
        } else if let Expression::ComputedMemberExpression(member) = &expression.callee {
            // `Object["defineProperty"]` is the same mutation as the static
            // member form; a string-literal computed callee must not bypass the
            // target/key barrier. Only the modelled mutation key is recognized.
            if matches!(&member.expression, Expression::StringLiteral(key) if key.value == "defineProperty")
            {
                self.record_member_target(
                    &member.object,
                    Some("defineProperty".to_string()),
                    Some(define_target_raw(expression, self.scoping)),
                    define_key_raw(expression),
                    expression.span,
                );
            }
        } else if let Expression::Identifier(identifier) = &expression.callee {
            if let Some(symbol) = analysis::identifier_symbol(identifier, self.scoping) {
                self.mutations.calls.push(CallMutation {
                    callee: symbol,
                    define_target: define_target_raw(expression, self.scoping),
                    span: expression.span,
                    write_key: define_key_raw(expression),
                });
            }
        }
        walk::walk_call_expression(self, expression);
    }
}

/// Raw shape of a `defineProperty` call's target (first argument). A missing or
/// spread argument is opaque and keeps the full barrier.
fn define_target_raw(expression: &CallExpression<'_>, scoping: &Scoping) -> RawExpr {
    expression
        .arguments
        .first()
        .and_then(|argument| argument.as_expression())
        .map(|argument| raw_expr(argument, scoping))
        .unwrap_or(RawExpr::Unknown)
}

/// Literal string key of a `defineProperty` call (second argument). A dynamic
/// key yields `None`, which keeps the unproven-key barrier.
fn define_key_raw(expression: &CallExpression<'_>) -> Option<String> {
    match expression
        .arguments
        .get(1)
        .and_then(|argument| argument.as_expression())
    {
        Some(Expression::StringLiteral(string)) => Some(string.value.to_string()),
        _ => None,
    }
}

fn opaque_property_target(expression: &Expression<'_>, scoping: &Scoping) -> bool {
    match expression {
        Expression::Identifier(identifier) => {
            analysis::identifier_symbol(identifier, scoping).is_none()
        }
        Expression::CallExpression(_) | Expression::ComputedMemberExpression(_) => true,
        Expression::StaticMemberExpression(member) => {
            let Some(path) = expression_path(expression) else {
                return true;
            };
            let root = path.split('.').next().unwrap_or_default();
            !analysis::is_native_global(root)
                && !matches!(root, "globalThis" | "window" | "self")
                && identifier_symbol(&member.object, scoping).is_none()
        }
        _ => false,
    }
}

impl MutationCollector<'_> {
    fn record(&mut self, path: String) {
        record_mutation_path(&mut self.mutations, &path);
    }

    fn record_assignment_target(&mut self, target: &AssignmentTarget<'_>, span: Span) {
        match target {
            AssignmentTarget::StaticMemberExpression(member) => {
                self.record_member_target(
                    &member.object,
                    Some(member.property.name.to_string()),
                    None,
                    None,
                    span,
                );
            }
            AssignmentTarget::ComputedMemberExpression(member) => {
                self.record_member_target(&member.object, None, None, None, span);
            }
            _ => {}
        }
    }

    fn record_member_target(
        &mut self,
        object: &Expression<'_>,
        property: Option<String>,
        define_target: Option<RawExpr>,
        write_key: Option<String>,
        span: Span,
    ) {
        let Some(path) = expression_path(object) else {
            return;
        };
        let object_symbol = expression_root_symbol(object, self.scoping);
        let object_name = expression_root_name(object);
        let path = property
            .as_ref()
            .map_or_else(|| path.clone(), |property| format!("{path}.{property}"));
        self.mutations.member_targets.push(MemberMutation {
            object_symbol,
            object_name,
            path,
            property,
            span,
            define_target,
            write_key,
        });
    }
}

fn assignment_target_path(target: &AssignmentTarget<'_>) -> Option<String> {
    match target {
        AssignmentTarget::StaticMemberExpression(member) => Some(format!(
            "{}.{}",
            expression_path(&member.object)?,
            member.property.name
        )),
        AssignmentTarget::ComputedMemberExpression(member) => {
            Some(format!("{}[]", expression_path(&member.object)?))
        }
        AssignmentTarget::AssignmentTargetIdentifier(identifier) => {
            Some(identifier.name.to_string())
        }
        _ => None,
    }
}

fn assignment_target_symbol(target: &AssignmentTarget<'_>, scoping: &Scoping) -> Option<SymbolId> {
    let object = match target {
        AssignmentTarget::StaticMemberExpression(member) => &member.object,
        AssignmentTarget::ComputedMemberExpression(member) => &member.object,
        _ => return None,
    };
    identifier_symbol(object, scoping)
}

fn simple_target_path(target: &SimpleAssignmentTarget<'_>) -> Option<String> {
    match target {
        SimpleAssignmentTarget::StaticMemberExpression(member) => Some(format!(
            "{}.{}",
            expression_path(&member.object)?,
            member.property.name
        )),
        SimpleAssignmentTarget::ComputedMemberExpression(member) => {
            Some(expression_path(&member.object)?)
        }
        SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier) => {
            Some(identifier.name.to_string())
        }
        _ => None,
    }
}

fn simple_target_symbol(
    target: &SimpleAssignmentTarget<'_>,
    scoping: &Scoping,
) -> Option<SymbolId> {
    let object = match target {
        SimpleAssignmentTarget::StaticMemberExpression(member) => &member.object,
        SimpleAssignmentTarget::ComputedMemberExpression(member) => &member.object,
        _ => return None,
    };
    identifier_symbol(object, scoping)
}

fn identifier_symbol(expression: &Expression<'_>, scoping: &Scoping) -> Option<SymbolId> {
    crate::analysis::expression_symbol(expression, scoping)
}

fn expression_root_symbol(expression: &Expression<'_>, scoping: &Scoping) -> Option<SymbolId> {
    match expression {
        Expression::Identifier(_) => identifier_symbol(expression, scoping),
        Expression::StaticMemberExpression(member) => {
            expression_root_symbol(&member.object, scoping)
        }
        Expression::ComputedMemberExpression(member) => {
            expression_root_symbol(&member.object, scoping)
        }
        _ => None,
    }
}

fn expression_root_name(expression: &Expression<'_>) -> Option<String> {
    match expression {
        Expression::Identifier(identifier) => Some(identifier.name.to_string()),
        Expression::StaticMemberExpression(member) => expression_root_name(&member.object),
        Expression::ComputedMemberExpression(member) => expression_root_name(&member.object),
        _ => None,
    }
}

fn expression_path(expression: &Expression<'_>) -> Option<String> {
    match expression {
        Expression::Identifier(identifier) => Some(identifier.name.to_string()),
        Expression::StaticMemberExpression(member) => Some(format!(
            "{}.{}",
            expression_path(&member.object)?,
            member.property.name
        )),
        Expression::ComputedMemberExpression(member) => {
            Some(format!("{}[]", expression_path(&member.object)?))
        }
        _ => None,
    }
}

fn is_mutating_native_call(expression: &Expression<'_>) -> bool {
    // `defineProperty` is handled separately so its target (first argument) can
    // be classified; these calls mutate the receiver itself and always take the
    // full barrier.
    matches!(expression, Expression::StaticMemberExpression(member) if matches!(&member.object, Expression::Identifier(identifier) if identifier.name == "Object" || identifier.name == "Reflect") && matches!(member.property.name.as_str(), "setPrototypeOf" | "preventExtensions" | "seal" | "freeze"))
}
