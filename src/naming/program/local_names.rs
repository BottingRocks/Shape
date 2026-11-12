//! Readability names for the generated local bindings left after the semantic
//! plan.
//!
//! The semantic naming plan names *function identities* and *main-owned
//! aliases*; by design it never names an ordinary local, a parameter, a
//! `catch` binding or a temporary. Recovered output therefore still contains
//! tracer spellings such as `$KEY_17`, `$A`, `$SSA_3`, `$CARRY_9`,
//! `$ARRAY_2_1` and `$BOOLEAN_4`. This pass gives those bindings readable,
//! **evidence-based** names.
//!
//! # What it is
//!
//! Every candidate is a binding declared inside a function body, a function
//! parameter or pattern binding, a `catch` parameter or a loop binding whose
//! current spelling is a generated `$`-prefixed name. The name comes from the
//! narrowest structural evidence available:
//!
//! - a `catch` parameter is `error`;
//! - a `for` loop counter with a numeric start and a test/update that reads it
//!   is `i`, `j`, `k`, … by nesting;
//! - a parameter called as a function is `callback`, one used as a collection
//!   (`.length`, a numeric element access, `push`) is `items`, one only read
//!   through properties is `record`, one used in string operations is `text`,
//!   and any other parameter is `value`;
//! - a variable follows its initializer: function/arrow → `callback` (or
//!   `handler` when it is called), array → `items`, object literal → `record`,
//!   `new` → `instance`, call → `result`, string → `text`, number/arithmetic →
//!   `value`, boolean → `flag`, `void 0` → `value`, anything else → `value`.
//!
//! A `$SSA_` or `$CARRY_` temporary with no initializer borrows the shape of
//! its single simple assignment. A pure forwarding copy has no stronger
//! evidence and is named `value`.
//!
//! This is a *readability pass*, not a semantic claim: a name never asserts
//! more than the initializer or use shows, and the neutral fallback is
//! `value`. It deliberately does not name function or class bindings (the
//! semantic plan owns those), program-scope/MAIN bindings, properties, labels,
//! `this`, `arguments`, `eval`, imports or free names.
//!
//! # Safety
//!
//! Edits are exact identifier spans taken from resolved [`SymbolId`]s, never
//! text search. Object and destructuring shorthands are expanded so the
//! property name is preserved. Within a scope (and against names visible to
//! nested functions) a name is suffixed `2`, `3`, … rather than shadowing or
//! capturing a binding or a free name. The pass fails closed: after the edit
//! batch it reparses the output, requires no parse or early errors, proves that
//! every identifier reference resolves to the same declaration as before,
//! compares the free-name sets, and confirms no generated local remains
//! unaccounted for. If any check fails the *whole* local batch is discarded and
//! the input source is returned with every outcome marked refused.
//!
//! A function containing a direct `eval` reference or a `with` statement keeps
//! its generated locals, and so does every enclosing function whose bindings a
//! nested `eval` could reach; those bindings are counted as refused.
//!
//! The pass is idempotent: after it every remaining local either has a
//! readable name or was refused, so a second run finds no generated candidate
//! and changes nothing.

use std::collections::{BTreeMap, BTreeSet};

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast::AstKind;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, NodeId, ScopeFlags, Scoping, SemanticBuilder, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::scope::ScopeId;

use crate::analysis;
use crate::signals::schema::SourceSpan;

use super::rename::{collect_binding_identifiers, collect_sites, is_valid_name, BindingSite};
use super::ProgramTransformError;

/// Options for [`apply_local_names`]. The pass is on by default because it is
/// a readability improvement over generated spellings; callers opt out with
/// [`LocalNameOptions::disabled`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalNameOptions {
    pub enabled: bool,
}

impl Default for LocalNameOptions {
    fn default() -> Self {
        Self { enabled: true }
    }
}

impl LocalNameOptions {
    /// The pass runs.
    pub const fn enabled() -> Self {
        Self { enabled: true }
    }

    /// The pass is a no-op and returns the input untouched.
    pub const fn disabled() -> Self {
        Self { enabled: false }
    }
}

/// The syntactic role of a local binding, for reporting only.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LocalBindingKind {
    /// A `var`/`let`/`const` binding (or a destructured part of one).
    Variable,
    /// A function parameter or pattern binding.
    Parameter,
    /// A `catch` parameter.
    CatchParameter,
    /// A `for` loop counter with a numeric start and test/update.
    LoopCounter,
    /// A `for`-`in`/`for`-`of` binding.
    LoopBinding,
    /// A class declaration created by an already-named factory function.
    ClassIdentity,
}

impl LocalBindingKind {
    /// A stable machine label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Variable => "variable",
            Self::Parameter => "parameter",
            Self::CatchParameter => "catch_parameter",
            Self::LoopCounter => "loop_counter",
            Self::LoopBinding => "loop_binding",
            Self::ClassIdentity => "class_identity",
        }
    }
}

/// Why one local binding kept its generated spelling.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LocalNameRefusal {
    /// The binding is in a function (or reachable from a nested function) with
    /// a direct `eval`/`with` dynamic scope.
    DynamicScope,
    /// No safe name was found without shadowing or capturing another name.
    NoSafeName,
    /// The binding site cannot be rewritten exactly (for example an import
    /// shorthand).
    UnsupportedBindingSite(String),
    /// The whole local edit batch failed post-edit validation.
    BatchRejected(String),
}

impl LocalNameRefusal {
    /// A stable machine label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::DynamicScope => "dynamic_scope",
            Self::NoSafeName => "no_safe_name",
            Self::UnsupportedBindingSite(_) => "unsupported_binding_site",
            Self::BatchRejected(_) => "batch_rejected",
        }
    }
}

/// One local binding's outcome.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalNameOutcome {
    /// The generated spelling before the pass.
    pub original_name: String,
    /// The proposed readable name, when one was produced.
    pub new_name: Option<String>,
    pub kind: LocalBindingKind,
    /// The structural evidence the name is based on.
    pub evidence: String,
    /// The declaring identifier's span in the input.
    pub declaration_span: SourceSpan,
    /// Whether the rename was applied.
    pub applied: bool,
    /// Why it was not applied.
    pub refusal: Option<LocalNameRefusal>,
    /// Reference sites rewritten.
    pub reference_edits: usize,
    /// Shorthand sites expanded (`{ x }` → `{ x: name }`).
    pub shorthand_edits: usize,
}

/// The result of [`apply_local_names`].
#[derive(Clone, Debug)]
pub struct LocalNamesResult {
    /// The input, unchanged when nothing was applied or the batch was refused.
    pub transformed_source: String,
    /// One outcome per generated local candidate, in declaration order.
    pub outcomes: Vec<LocalNameOutcome>,
    /// Bindings renamed.
    pub renamed: usize,
    /// Bindings left generated (dynamic scope, no safe name, batch refusal).
    pub refused: usize,
    /// Generated local bindings still present in `transformed_source`.
    pub remaining: usize,
    /// The reason the whole batch was discarded, when it was.
    pub batch_rejection: Option<String>,
    pub options: LocalNameOptions,
}

impl LocalNamesResult {
    /// True when the pass changed the source.
    pub fn changed(&self) -> bool {
        self.renamed > 0
    }

    /// Look up a binding's outcome by its generated spelling.
    pub fn outcome(&self, original: &str) -> Option<&LocalNameOutcome> {
        self.outcomes
            .iter()
            .find(|outcome| outcome.original_name == original)
    }
}

/// Rename generated local bindings in `source` with readable, evidence-based
/// names.
///
/// Only parsing the input can fail outright; every rewrite or validation
/// problem is reported in the result, which keeps the input text.
pub fn apply_local_names(
    source: &str,
    options: LocalNameOptions,
) -> Result<LocalNamesResult, ProgramTransformError> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source).map_err(ProgramTransformError::Parse)?;
    let (scoping, nodes) = analysis::analyze(&program);

    let sites = collect_binding_identifiers(&program);
    let candidates = collect_candidates(&program, &scoping, &nodes, &sites);

    if !options.enabled {
        return Ok(LocalNamesResult {
            transformed_source: source.to_owned(),
            outcomes: Vec::new(),
            renamed: 0,
            refused: 0,
            remaining: candidates.len(),
            batch_rejection: None,
            options,
        });
    }

    let source_refs = reference_records(&program, &scoping);
    let proposed = assign_names(&candidates, &scoping, &nodes, &program);
    let mut outcomes = Vec::with_capacity(candidates.len());
    let mut edits = Vec::new();
    let mut renamed_targets: BTreeMap<u32, String> = BTreeMap::new();
    let mut renamed = 0usize;

    for (candidate, proposed) in candidates.iter().zip(proposed.iter()) {
        let mut outcome = LocalNameOutcome {
            original_name: candidate.original.clone(),
            new_name: proposed.clone(),
            kind: candidate.kind,
            evidence: candidate.evidence.clone(),
            declaration_span: SourceSpan::new(
                candidate.declaration_start,
                candidate.declaration_span.end,
            ),
            applied: false,
            refusal: None,
            reference_edits: 0,
            shorthand_edits: 0,
        };
        if let Some(refusal) = &candidate.refusal {
            outcome.refusal = Some(refusal.clone());
            outcomes.push(outcome);
            continue;
        }
        let Some(new_name) = proposed else {
            outcome.refusal = Some(LocalNameRefusal::NoSafeName);
            outcomes.push(outcome);
            continue;
        };
        match collect_sites(
            &scoping,
            &nodes,
            &sites,
            candidate.symbol,
            &candidate.original,
            new_name,
        ) {
            Ok((site_edits, references, shorthands)) => {
                edits.extend(site_edits);
                outcome.applied = true;
                outcome.reference_edits = references;
                outcome.shorthand_edits = shorthands;
                renamed += 1;
                renamed_targets.insert(candidate.declaration_start, new_name.clone());
            }
            Err((reason, _)) => {
                outcome.refusal = Some(LocalNameRefusal::UnsupportedBindingSite(reason.to_string()));
            }
        }
        outcomes.push(outcome);
    }

    if edits.is_empty() {
        let refused = outcomes.iter().filter(|outcome| !outcome.applied).count();
        return Ok(LocalNamesResult {
            transformed_source: source.to_owned(),
            outcomes,
            renamed: 0,
            refused,
            remaining: candidates.len(),
            batch_rejection: None,
            options,
        });
    }

    // `unshift` maps an output offset back to the input by replaying the
    // edits in source order; keep that order even though edits are collected
    // per binding (declaration first, then its references).
    let mut edit_info: Vec<EditInfo> = edits
        .iter()
        .map(|edit| EditInfo {
            start: edit.start,
            end: edit.end,
            new_len: edit.replacement.len() as u32,
        })
        .collect();
    edit_info.sort_by_key(|edit| (edit.start, edit.end));
    let applied = analysis::apply_edits(source, edits);
    if !applied.rejected.is_empty() {
        return Ok(reject_batch(
            source,
            outcomes,
            candidates.len(),
            format!(
                "{} local edit(s) overlapped; no local edit was applied",
                applied.rejected.len()
            ),
            options,
        ));
    }

    let remaining = match validate_rewrite(
        &source_refs,
        &applied.output,
        &edit_info,
        &renamed_targets,
    ) {
        Ok(remaining) => remaining,
        Err(reason) => {
            return Ok(reject_batch(
                source,
                outcomes,
                candidates.len(),
                reason,
                options,
            ))
        }
    };
    let refused = outcomes.iter().filter(|outcome| !outcome.applied).count();
    if remaining != refused {
        return Ok(reject_batch(
            source,
            outcomes,
            candidates.len(),
            format!(
                "{remaining} generated local binding(s) remain but {refused} were reported refused"
            ),
            options,
        ));
    }

    Ok(LocalNamesResult {
        transformed_source: applied.output,
        outcomes,
        renamed,
        refused,
        remaining,
        batch_rejection: None,
        options,
    })
}

/// Discard the batch: keep the input text and mark every applied outcome
/// refused with one reason.
fn reject_batch(
    source: &str,
    mut outcomes: Vec<LocalNameOutcome>,
    remaining: usize,
    reason: String,
    options: LocalNameOptions,
) -> LocalNamesResult {
    for outcome in &mut outcomes {
        if outcome.applied {
            outcome.applied = false;
            outcome.refusal = Some(LocalNameRefusal::BatchRejected(reason.clone()));
        }
    }
    LocalNamesResult {
        transformed_source: source.to_owned(),
        refused: outcomes.len(),
        renamed: 0,
        remaining,
        batch_rejection: Some(reason),
        options,
        outcomes,
    }
}

/// A generated local spelling. The recovered/emitted program prefixes every
/// temporary it invents with `$`; original bindings from the capture that are
/// already readable never match.
fn is_generated_name(name: &str) -> bool {
    name.len() > 1 && name.starts_with('$')
}

struct Candidate {
    symbol: SymbolId,
    original: String,
    kind: LocalBindingKind,
    scope: ScopeId,
    declaration_start: u32,
    declaration_span: Span,
    /// `<base>: <explanation>`, or a neutral explanation with no base.
    evidence: String,
    /// A numeric `for` initializer, when the binding is an initialized loop
    /// variable.
    loop_node: Option<NodeId>,
    counter_depth: usize,
    /// Set when the binding must stay generated.
    refusal: Option<LocalNameRefusal>,
    /// The symbol this binding is a direct copy of, when its initializer is a
    /// plain identifier (`let x = y`).
    from: Option<SymbolId>,
}

/// What a binding site's nearest declaration context is.
struct Classified {
    kind: LocalBindingKind,
    owner: Span,
    init: Option<(String, String)>,
    loop_node: Option<NodeId>,
    destructured: bool,
    from: Option<SymbolId>,
    owner_name: Option<String>,
}

/// Collect every generated local binding declared inside a function.
fn collect_candidates(
    program: &Program,
    scoping: &Scoping,
    nodes: &AstNodes,
    sites: &[BindingSite],
) -> Vec<Candidate> {
    let dynamics = dynamic_functions(program);
    let refs = reference_nodes(program, scoping);
    let writes = assignment_shapes(program, scoping);

    let mut by_symbol: BTreeMap<SymbolId, Vec<&BindingSite>> = BTreeMap::new();
    for site in sites {
        by_symbol.entry(site.symbol).or_default().push(site);
    }

    let mut candidates = Vec::new();
    for (symbol, mut symbol_sites) in by_symbol {
        let original = scoping.symbol_name(symbol).to_owned();
        if !is_generated_name(&original) {
            continue;
        }
        symbol_sites.sort_by_key(|site| site.span.start);
        let Some(classified) = classify_binding(nodes, scoping, symbol_sites[0]) else {
            continue;
        };
        let symbol_refs = refs.get(&symbol).map(Vec::as_slice).unwrap_or(&[]);
        let returned = symbol_refs.iter().any(|node| {
            matches!(nodes.parent_kind(*node), AstKind::ReturnStatement(_))
        });

        let (kind, evidence) = if classified.kind == LocalBindingKind::LoopCounter {
            if is_loop_counter(nodes, symbol_refs, classified.loop_node) {
                (LocalBindingKind::LoopCounter, "numeric for-loop counter".to_string())
            } else if let Some((base, evidence)) = &classified.init {
                (LocalBindingKind::Variable, evidence_with_base(evidence, base))
            } else {
                (LocalBindingKind::Variable, "numeric loop variable".to_string())
            }
        } else if classified.kind == LocalBindingKind::Parameter {
            let (mut base, evidence) = parameter_evidence(nodes, symbol_refs);
            if is_weak_base(&base)
                && classified
                    .owner_name
                    .as_deref()
                    .is_some_and(callback_owner)
            {
                base = "callback".to_string();
            }
            (classified.kind, evidence_with_base(&evidence, &base))
        } else if classified.kind == LocalBindingKind::CatchParameter {
            (
                LocalBindingKind::CatchParameter,
                "catch parameter".to_string(),
            )
        } else if classified.kind == LocalBindingKind::LoopBinding {
            (
                classified.kind,
                "for-in/of binding; neutral fallback".to_string(),
            )
        } else if let Some((base, evidence)) = &classified.init {
            // A `void 0`/`0`/`null` initializer says nothing; the variable's
            // content arrives in later assignments, so name it from those.
            let (base, evidence) = if is_weak_base(base) {
                writes
                    .get(&symbol)
                    .and_then(|shapes| {
                        combined_shape(shapes).or_else(|| single_shape(shapes))
                    })
                    .unwrap_or_else(|| {
                        if evidence.contains("void") {
                            ("placeholder".into(), "declared without a value".into())
                        } else {
                            (base.clone(), evidence.clone())
                        }
                    })
            } else {
                (base.clone(), evidence.clone())
            };
            (classified.kind, evidence_with_base(&evidence, &base))
        } else if is_temporary(&original) {
            match writes
                .get(&symbol)
                .and_then(|shapes| combined_shape(shapes).or_else(|| single_shape(shapes)))
            {
                Some((base, evidence)) => {
                    (classified.kind, evidence_with_base(&evidence, &base))
                }
                None => (
                    classified.kind,
                    "temporary with no single assignment; neutral fallback".to_string(),
                ),
            }
        } else if classified.destructured {
            (
                classified.kind,
                "destructured binding; neutral fallback".to_string(),
            )
        } else if let Some((base, evidence)) = writes
            .get(&symbol)
            .and_then(|shapes| combined_shape(shapes).or_else(|| single_shape(shapes)))
        {
            // `let x;` carries no type, but its assignments do.
            (classified.kind, evidence_with_base(&evidence, &base))
        } else if is_temporary(&original) {
            (
                classified.kind,
                "temporary with no initializer; neutral fallback".to_string(),
            )
        } else {
            (
                classified.kind,
                "no initializer or assignment; neutral fallback".to_string(),
            )
        };

        // A weak local the function returns is the function's result.
        let (kind, evidence) = if returned && evidence.starts_with("value:") {
            (kind, "result: returned from the function".to_string())
        } else {
            (kind, evidence)
        };
        let dynamic = dynamics
            .iter()
            .any(|dynamic| contains(classified.owner, *dynamic));
        candidates.push(Candidate {
            symbol,
            original,
            kind,
            scope: scoping.symbol_scope_id(symbol),
            declaration_start: symbol_sites[0].span.start,
            declaration_span: symbol_sites[0].span,
            evidence,
            loop_node: classified.loop_node,
            counter_depth: 0,
            refusal: dynamic.then_some(LocalNameRefusal::DynamicScope),
            from: classified.from,
        });
    }

    candidates.sort_by_key(|candidate| candidate.declaration_start);
    apply_counter_depth(&mut candidates, nodes);
    candidates
}

fn evidence_with_base(evidence: &str, base: &str) -> String {
    format!("{base}: {evidence}")
}

/// The nearest declaration context of a binding identifier.
///
/// Stops at the owning function/arrow (returning its span) or at an enclosing
/// class when there is no function in between (not a local).
fn classify_binding(nodes: &AstNodes, scoping: &Scoping, site: &BindingSite) -> Option<Classified> {
    let mut kind: Option<LocalBindingKind> = None;
    let mut init = None;
    let mut loop_node = None;
    let mut destructured = false;
    let mut from: Option<SymbolId> = None;
    let mut node = site.node;
    for _ in 0..256 {
        let parent = nodes.parent_id(node);
        if parent == node {
            return None;
        }
        match nodes.kind(parent) {
            AstKind::VariableDeclarator(declarator) => {
                if kind.is_none() {
                    kind = Some(LocalBindingKind::Variable);
                    destructured = declarator.id.span() != site.span;
                    if !destructured {
                        if let Some(expression) = declarator.init.as_ref() {
                            init = Some(expression_shape(expression));
                            if let Expression::Identifier(identifier) = expression.without_parentheses() {
                                from = analysis::identifier_symbol(identifier, scoping);
                            }
                        }
                    }
                    let declaration = nodes.parent_id(parent);
                    let container = nodes.parent_id(declaration);
                    if let AstKind::ForStatement(_) = nodes.kind(container) {
                        if is_for_initializer(nodes, container, declarator.id.span()) {
                            loop_node = Some(container);
                            kind = Some(LocalBindingKind::LoopCounter);
                        }
                    } else if matches!(
                        nodes.kind(container),
                        AstKind::ForInStatement(_) | AstKind::ForOfStatement(_)
                    ) {
                        kind = Some(LocalBindingKind::LoopBinding);
                    }
                }
            }
            AstKind::FormalParameter(_) if kind.is_none() => {
                kind = Some(LocalBindingKind::Parameter);
            }
            AstKind::CatchParameter(_) if kind.is_none() => {
                kind = Some(LocalBindingKind::CatchParameter);
            }
            AstKind::BindingProperty(_)
            | AstKind::ArrayPattern(_)
            | AstKind::ObjectPattern(_)
            | AstKind::AssignmentPattern(_)
            | AstKind::BindingRestElement(_) => {
                destructured = true;
            }
            AstKind::Class(class) => {
                if kind.is_some() {
                    return None;
                }
                let is_name = class
                    .id
                    .as_ref()
                    .is_some_and(|identifier| identifier.span == site.span);
                if !is_name {
                    return None;
                }
                // The class name is a function identity the semantic plan did
                // not claim. Name it after the already-named factory that
                // creates it: a structural fact, not a guessed role.
                let mut ancestor = nodes.parent_id(parent);
                let mut factory = None;
                for _ in 0..256 {
                    match nodes.kind(ancestor) {
                        AstKind::Function(function) => {
                            if let Some(identifier) = &function.id {
                                factory = Some(identifier.name.to_string());
                            }
                            break;
                        }
                        AstKind::ArrowFunctionExpression(_) => break,
                        _ => {}
                    }
                    let next = nodes.parent_id(ancestor);
                    if next == ancestor {
                        break;
                    }
                    ancestor = next;
                }
                let Some(factory) = factory else {
                    return None;
                };
                let base = class_base_from_factory(&factory);
                return Some(Classified {
                    kind: LocalBindingKind::ClassIdentity,
                    owner: class.span,
                    init: Some((base, format!("class defined by `{factory}`"))),
                    loop_node: None,
                    destructured: false,
                    from: None,
                    owner_name: Some(factory.clone()),
                });
            }
            AstKind::Function(function) => {
                if function
                    .id
                    .as_ref()
                    .is_some_and(|identifier| identifier.span == site.span)
                {
                    return None;
                }
                return kind.map(|kind| Classified {
                    kind,
                    owner: function.span,
                    init,
                    loop_node,
                    destructured,
                    from,
                    owner_name: function.id.as_ref().map(|id| id.name.to_string()),
                });
            }
            AstKind::ArrowFunctionExpression(arrow) => {
                return kind.map(|kind| Classified {
                    kind,
                    owner: arrow.span,
                    init,
                    loop_node,
                    destructured,
                    from,
                    owner_name: enclosing_function_name(nodes, parent),
                });
            }
            AstKind::ImportSpecifier(_)
            | AstKind::ImportDefaultSpecifier(_)
            | AstKind::ImportNamespaceSpecifier(_)
            | AstKind::ExportSpecifier(_) => return None,
            _ => {}
        }
        node = parent;
    }
    None
}

/// Is this declarator part of a `for` statement's initializer (as opposed to a
/// declaration in the loop body)?
fn is_for_initializer(nodes: &AstNodes, for_node: NodeId, pattern: Span) -> bool {
    let AstKind::ForStatement(statement) = nodes.kind(for_node) else {
        return false;
    };
    let Some(ForStatementInit::VariableDeclaration(declaration)) = statement.init.as_ref() else {
        return false;
    };
    declaration
        .declarations
        .iter()
        .any(|declarator| declarator.id.span() == pattern)
}

/// Numeric start plus a test and update that read the binding. The update may
/// be `i++`, `++i` or `i += n`; without both a test and an update the binding
/// is an ordinary variable whose numeric initializer alone is not enough to
/// call it a counter.
fn is_loop_counter(nodes: &AstNodes, refs: &[NodeId], loop_node: Option<NodeId>) -> bool {
    let Some(loop_node) = loop_node else {
        return false;
    };
    let AstKind::ForStatement(statement) = nodes.kind(loop_node) else {
        return false;
    };
    let (Some(test), Some(update)) = (statement.test.as_ref(), statement.update.as_ref()) else {
        return false;
    };
    let test = test.span();
    let update = update.span();
    let in_test = refs
        .iter()
        .any(|node| contains(test, nodes.kind(*node).span()));
    let in_update = refs
        .iter()
        .any(|node| contains(update, nodes.kind(*node).span()));
    in_test && in_update
}

fn apply_counter_depth(candidates: &mut [Candidate], nodes: &AstNodes) {
    let counter_loops: BTreeSet<NodeId> = candidates
        .iter()
        .filter(|candidate| {
            candidate.kind == LocalBindingKind::LoopCounter && candidate.refusal.is_none()
        })
        .filter_map(|candidate| candidate.loop_node)
        .collect();
    for candidate in candidates.iter_mut() {
        if candidate.kind != LocalBindingKind::LoopCounter || candidate.refusal.is_some() {
            continue;
        }
        let Some(loop_node) = candidate.loop_node else {
            continue;
        };
        let mut depth = 0;
        let mut node = loop_node;
        loop {
            let parent = nodes.parent_id(node);
            if parent == node {
                break;
            }
            if matches!(
                nodes.kind(parent),
                AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)
            ) {
                break;
            }
            if counter_loops.contains(&parent) {
                depth += 1;
            }
            node = parent;
        }
        candidate.counter_depth = depth;
    }
}

/// The shape of a variable initializer (base name, evidence).
fn expression_shape(expression: &Expression) -> (String, String) {
    match expression {
        Expression::FunctionExpression(_) => ("callback".into(), "function expression".into()),
        Expression::ArrowFunctionExpression(_) => ("callback".into(), "arrow function".into()),
        Expression::ArrayExpression(_) => ("items".into(), "array literal".into()),
        Expression::ObjectExpression(_) => ("record".into(), "object literal".into()),
        Expression::NewExpression(new) => (
            new_base(&new.callee),
            format!("new {}", expression_text(&new.callee)),
        ),
        Expression::CallExpression(call) => (
            call_base(&call.callee),
            format!("call to {}", expression_text(&call.callee)),
        ),
        Expression::StringLiteral(_) | Expression::TemplateLiteral(_) => {
            ("text".into(), "string literal".into())
        }
        Expression::NumericLiteral(_) | Expression::BigIntLiteral(_) => {
            ("value".into(), "numeric literal".into())
        }
        Expression::BooleanLiteral(_) => ("flag".into(), "boolean literal".into()),
        Expression::NullLiteral(_) => ("value".into(), "null literal".into()),
        Expression::UnaryExpression(unary) => match unary.operator {
            UnaryOperator::Void => ("value".into(), "void expression".into()),
            UnaryOperator::LogicalNot => ("flag".into(), "boolean negation".into()),
            UnaryOperator::Typeof => ("text".into(), "typeof string".into()),
            UnaryOperator::UnaryPlus
            | UnaryOperator::UnaryNegation
            | UnaryOperator::BitwiseNot => ("value".into(), "numeric unary expression".into()),
            _ => ("value".into(), "unknown unary expression".into()),
        },
        Expression::BinaryExpression(binary) => {
            if is_comparison(binary.operator) || equality(binary.operator) {
                ("flag".into(), "comparison result".into())
            } else {
                let base = match binary.operator {
                    BinaryOperator::Addition => "sum",
                    BinaryOperator::Subtraction => "delta",
                    BinaryOperator::Multiplication => "product",
                    BinaryOperator::Division => "ratio",
                    BinaryOperator::Remainder => "remainder",
                    BinaryOperator::BitwiseOR
                    | BinaryOperator::BitwiseXOR
                    | BinaryOperator::BitwiseAnd
                    | BinaryOperator::ShiftLeft
                    | BinaryOperator::ShiftRight
                    | BinaryOperator::ShiftRightZeroFill => "hash",
                    _ => "value",
                };
                (base.into(), "arithmetic/bitwise expression".into())
            }
        }
        Expression::ConditionalExpression(conditional) => {
            let (left, _) = expression_shape(&conditional.consequent);
            let (right, _) = expression_shape(&conditional.alternate);
            if left == right {
                (left, "both conditional branches".into())
            } else {
                ("value".into(), "conditional expression".into())
            }
        }
        Expression::LogicalExpression(logical) => {
            let (left, _) = expression_shape(&logical.left);
            let (right, _) = expression_shape(&logical.right);
            if left == right {
                (left, "both logical operands".into())
            } else {
                (left, "logical expression".into())
            }
        }
        Expression::SequenceExpression(sequence) => sequence
            .expressions
            .last()
            .map(expression_shape)
            .unwrap_or_else(|| ("value".into(), "empty sequence".into())),
        Expression::AssignmentExpression(assignment) => expression_shape(&assignment.right),
        Expression::AwaitExpression(_) => ("result".into(), "awaited result".into()),
        Expression::YieldExpression(_) => ("result".into(), "yielded result".into()),
        Expression::StaticMemberExpression(member) => (
            member_base(member.property.name.as_str()),
            format!("reads .{}", member.property.name),
        ),
        Expression::ComputedMemberExpression(member) => match &member.expression {
            Expression::StringLiteral(literal) => (
                member_base(literal.value.as_str()),
                format!("reads ['{}']", literal.value),
            ),
            _ => ("value".into(), "computed member read".into()),
        },
        Expression::Identifier(identifier) => (
            identifier_base(identifier.name.as_str()).unwrap_or_else(|| "value".into()),
            "forwarded identifier".into(),
        ),
        Expression::ChainExpression(_) => ("result".into(), "chained expression".into()),
        Expression::ClassExpression(_) => ("value".into(), "class expression".into()),
        Expression::ThisExpression(_)
        | Expression::TaggedTemplateExpression(_)
        | Expression::ParenthesizedExpression(_) => {
            ("value".into(), "compound expression".into())
        }
        _ => ("value".into(), "unknown initializer".into()),
    }
}


/// A readable noun for a value, from what the source actually names.
fn member_base(name: &str) -> String {
    match name {
        "length" | "size" | "count" => "count".into(),
        "global" | "window" | "self" | "globalThis" => "globalObject".into(),
        "document" => "document".into(),
        "navigator" => "navigator".into(),
        "location" => "location".into(),
        "data" => "data".into(),
        "status" => "status".into(),
        "type" => "type".into(),
        "name" => "name".into(),
        "message" => "message".into(),
        "stack" => "stack".into(),
        "keys" => "keys".into(),
        "entries" => "entries".into(),
        "href" | "url" => "url".into(),
        "token" | "transactionToken" => "token".into(),
        "id" => "id".into(),
        "value" | "valueOf" => "value".into(),
        "index" | "idx" => "index".into(),
        "random" => "random".into(),
        other => {
            let base = lower_camel(strip_accessor_prefix(other));
            if is_generated_name(&base) { "value".into() } else { base }
        }
    }
}

/// A readable noun for a call result, from the callee's own name.
fn call_base(callee: &Expression) -> String {
    let name = match callee.without_parentheses() {
        Expression::StaticMemberExpression(member) => member.property.name.as_str(),
        Expression::ComputedMemberExpression(member) => match &member.expression {
            Expression::StringLiteral(literal) => literal.value.as_str(),
            _ => return "result".into(),
        },
        Expression::Identifier(identifier) => identifier.name.as_str(),
        _ => return "result".into(),
    };
    let base = match name {
        "getContext" => "context".into(),
        "querySelectorAll" | "getElementsByTagName" | "getElementsByClassName" => "elements".into(),
        "querySelector" | "getElementById" | "createElement" => "element".into(),
        "getImageData" => "pixels".into(),
        "toDataURL" | "toBlob" => "dataUrl".into(),
        "random" => "random".into(),
        "now" => "timestamp".into(),
        "stringify" => "text".into(),
        "parse" => "parsed".into(),
        "map" => "mapped".into(),
        "filter" => "filtered".into(),
        "reduce" => "reduced".into(),
        "slice" => "slice".into(),
        "join" | "concat" => "text".into(),
        "hash" | "digest" => "hash".into(),
        "exec" | "match" => "match".into(),
        "test" => "flag".into(),
        "bind" => "callback".into(),
        "call" | "apply" => {
            if let Expression::StaticMemberExpression(outer) = callee.without_parentheses() {
                if let Expression::StaticMemberExpression(inner) =
                    outer.object.without_parentheses()
                {
                    return invoked_base(inner.property.name.as_str());
                }
                if let Expression::Identifier(identifier) = outer.object.without_parentheses() {
                    return invoked_base(identifier.name.as_str());
                }
            }
            "result".into()
        }
        "encode" => "encoded".into(),
        "decode" => "decoded".into(),
        "encrypt" => "encrypted".into(),
        "decrypt" => "decrypted".into(),
        "createEvent" => "event".into(),
        "sin" => "sine".into(),
        "cos" => "cosine".into(),
        "tan" => "tangent".into(),
        "floor" => "floored".into(),
        "ceil" => "ceiled".into(),
        "round" => "rounded".into(),
        "abs" => "absolute".into(),
        "max" => "maximum".into(),
        "min" => "minimum".into(),
        "pow" => "power".into(),
        "sqrt" => "squareRoot".into(),
        "log" => "logarithm".into(),
        "fround" => "float32".into(),
        "imul" => "product".into(),
        "trunc" => "truncated".into(),
        other => call_verb_base(other),
    };
    if is_generated_name(&base) { "result".into() } else { base }
}

/// The value kind of a function invoked through `.call`/`.apply`.
fn invoked_base(name: &str) -> String {
    match name {
        "hasOwnProperty" | "propertyIsEnumerable" | "isPrototypeOf" | "includes" => {
            "flag".into()
        }
        "toString" | "toLocaleString" => "text".into(),
        "valueOf" => "value".into(),
        "push" | "unshift" => "count".into(),
        "pop" | "shift" => "item".into(),
        "slice" | "splice" => "items".into(),
        other => call_verb_base(other),
    }
}

fn call_verb_base(name: &str) -> String {
    for (prefix, suffix) in [
        ("create", ""),
        ("build", ""),
        ("make", ""),
        ("get", ""),
        ("read", ""),
        ("parse", "Parsed"),
        ("encode", "Encoded"),
        ("decode", "Decoded"),
        ("encrypt", "Encrypted"),
        ("decrypt", "Decrypted"),
        ("is", ""),
        ("has", ""),
    ] {
        if let Some(rest) = name.strip_prefix(prefix) {
            if !rest.is_empty() {
                return lower_camel(rest) + suffix;
            }
        }
    }
    let base = lower_camel(name);
    let base = base.strip_suffix("Factory").unwrap_or(&base).to_string();
    // A lowercase, verb-less callee is a variable holding a function; its call
    // result is a result, not a value named after the variable.
    if name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
        return "result".into();
    }
    if is_generated_name(&base) { "result".into() } else { base }
}

fn new_base(callee: &Expression) -> String {
    let name = match callee.without_parentheses() {
        Expression::Identifier(identifier) => identifier.name.as_str(),
        Expression::StaticMemberExpression(member) => member.property.name.as_str(),
        _ => return "instance".into(),
    };
    match name {
        "Date" => "date".into(),
        "CustomEvent" | "Event" => "event".into(),
        "Uint8Array" | "Uint8ClampedArray" | "ArrayBuffer" => "bytes".into(),
        "Uint32Array" | "Int32Array" => "words".into(),
        "Float32Array" | "Float64Array" => "floats".into(),
        "Array" => "items".into(),
        "Map" | "WeakMap" => "map".into(),
        "Set" | "WeakSet" => "set".into(),
        "Error" | "TypeError" | "RangeError" => "error".into(),
        "RegExp" => "pattern".into(),
        "URL" => "url".into(),
        "XMLHttpRequest" => "request".into(),
        "Promise" => "promise".into(),
        "TextEncoder" => "encoder".into(),
        "TextDecoder" => "decoder".into(),
        "Blob" => "blob".into(),
        "FormData" => "formData".into(),
        "Worker" => "worker".into(),
        "Image" => "image".into(),
        "AudioContext" => "audioContext".into(),
        "MutationObserver" | "IntersectionObserver" => "observer".into(),
        "Proxy" => "proxy".into(),
        other => {
            let base = lower_camel(other);
            if is_generated_name(&base) { "instance".into() } else { base }
        }
    }
}

fn identifier_base(name: &str) -> Option<String> {
    if is_generated_name(name) || is_weak_base(name) {
        return None;
    }
    Some(lower_camel(strip_accessor_prefix(name)))
}

fn is_weak_base(name: &str) -> bool {
    matches!(name, "value" | "result" | "instance" | "data" | "other" | "placeholder")
        || name.strip_prefix("value").is_some_and(|rest| rest.chars().all(|c| c.is_ascii_digit()) && !rest.is_empty())
}

fn strip_accessor_prefix(name: &str) -> &str {
    for prefix in ["get", "set", "read", "write", "is", "has"] {
        if let Some(rest) = name.strip_prefix(prefix) {
            if rest.chars().next().is_some_and(|c| c.is_ascii_uppercase()) {
                return rest;
            }
        }
    }
    name
}

fn lower_camel(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first.to_lowercase().collect::<String>() + chars.as_str(),
        None => "value".into(),
    }
}

fn expression_text(expression: &Expression) -> String {
    match expression {
        Expression::Identifier(identifier) => identifier.name.to_string(),
        Expression::StaticMemberExpression(member) => member.property.name.to_string(),
        Expression::CallExpression(call) => call_base(&call.callee),
        Expression::NewExpression(new) => format!("new {}", expression_text(&new.callee)),
        _ => "expression".to_string(),
    }
}

/// The most frequent non-weak shape across several writes, when one exists.
fn combined_shape(shapes: &[(String, String)]) -> Option<(String, String)> {
    let mut counts: BTreeMap<&str, (usize, &(String, String))> = BTreeMap::new();
    for shape in shapes {
        if is_weak_base(&shape.0) {
            continue;
        }
        let entry = counts.entry(shape.0.as_str()).or_insert((0, shape));
        entry.0 += 1;
    }
    let best = counts.values().max_by_key(|(count, _)| *count)?;
    Some(best.1.clone())
}

fn single_shape(shapes: &[(String, String)]) -> Option<(String, String)> {
    match shapes {
        [shape] => Some(shape.clone()),
        _ => None,
    }
}

/// The shape of every simple `=` assignment's right-hand side, by target
/// symbol.
fn assignment_shapes(
    program: &Program,
    scoping: &Scoping,
) -> BTreeMap<SymbolId, Vec<(String, String)>> {
    struct Collect<'s> {
        scoping: &'s Scoping,
        shapes: BTreeMap<SymbolId, Vec<(String, String)>>,
    }
    impl<'s, 'a> Visit<'a> for Collect<'s> {
        fn visit_assignment_expression(&mut self, assignment: &AssignmentExpression<'a>) {
            walk::walk_assignment_expression(self, assignment);
            let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left else {
                return;
            };
            let Some(symbol) = analysis::identifier_symbol(identifier, self.scoping) else {
                return;
            };
            let shape = match assignment.operator {
                AssignmentOperator::Assign => expression_shape(&assignment.right),
                AssignmentOperator::Addition => (
                    "total".into(),
                    "accumulated addition".into(),
                ),
                AssignmentOperator::BitwiseAnd
                | AssignmentOperator::BitwiseOR
                | AssignmentOperator::BitwiseXOR
                | AssignmentOperator::ShiftLeft
                | AssignmentOperator::ShiftRight
                | AssignmentOperator::ShiftRightZeroFill => {
                    ("hash".into(), "bitwise accumulator".into())
                }
                _ => return,
            };
            self.shapes.entry(symbol).or_default().push(shape);
        }

        fn visit_update_expression(&mut self, update: &UpdateExpression<'a>) {
            walk::walk_update_expression(self, update);
            if let SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier) =
                &update.argument
            {
                if let Some(symbol) = analysis::identifier_symbol(identifier, self.scoping) {
                    self.shapes
                        .entry(symbol)
                        .or_default()
                        .push(("count".into(), "incremented counter".into()));
                }
            }
        }
    }
    let mut collect = Collect {
        scoping,
        shapes: BTreeMap::new(),
    };
    collect.visit_program(program);
    collect.shapes
}

fn is_temporary(name: &str) -> bool {
    name.starts_with("$SSA_") || name.starts_with("$CARRY_")
}

/// The parameter name evidence: called, collection, property-only, string or
/// arithmetic use. The first matching row wins, so `callback` beats `items`.
fn parameter_evidence(nodes: &AstNodes, refs: &[NodeId]) -> (String, String) {
    let mut called = false;
    let mut event = false;
    let mut error = false;
    let mut callback_arg = false;
    let mut item_arg = false;
    let mut event_arg = false;
    let mut items = false;
    let mut string_ops = false;
    let mut arithmetic = false;
    let mut only_property_reads = true;
    let mut any = false;

    for node in refs {
        any = true;
        let span = nodes.kind(*node).span();
        let parent = nodes.parent_id(*node);
        match nodes.kind(parent) {
            AstKind::CallExpression(call) if call.callee.span() == span => {
                called = true;
                only_property_reads = false;
            }
            AstKind::CallExpression(call)
                if call.arguments.iter().any(|argument| argument.span() == span) =>
            {
                only_property_reads = false;
                if let Some(name) = callee_name(&call.callee) {
                    match name {
                        "addEventListener" | "removeEventListener" | "dispatchEvent" => {
                            event_arg = true
                        }
                        "map" | "forEach" | "filter" | "some" | "every" | "find"
                        | "findIndex" | "reduce" | "reduceRight" | "sort" => item_arg = true,
                        "then" | "finally" | "done" => callback_arg = true,
                        "catch" | "fail" => error = true,
                        "setTimeout" | "setInterval" | "requestAnimationFrame"
                        | "queueMicrotask" => callback_arg = true,
                        _ => {}
                    }
                }
            }
            AstKind::StaticMemberExpression(member) if member.object.span() == span => {
                let property = member.property.name.as_str();
                let member_span = nodes.kind(parent).span();
                let member_call = matches!(
                    nodes.parent_kind(parent),
                    AstKind::CallExpression(call) if call.callee.span() == member_span
                );
                if property == "length" {
                    items = true;
                }
                if is_event_member(property) {
                    event = true;
                }
                if matches!(property, "message" | "stack" | "lineNumber") {
                    error = true;
                }
                if member_call {
                    only_property_reads = false;
                    if matches!(property, "call" | "apply") {
                        called = true;
                    }
                    if is_collection_method(property) {
                        items = true;
                    } else if is_string_method(property) {
                        string_ops = true;
                    } else if is_numeric_method(property) {
                        arithmetic = true;
                    }
                }
            }
            AstKind::ComputedMemberExpression(member) if member.object.span() == span => {
                if is_numeric(&member.expression) {
                    items = true;
                }
                let member_span = nodes.kind(parent).span();
                let member_call = matches!(
                    nodes.parent_kind(parent),
                    AstKind::CallExpression(call) if call.callee.span() == member_span
                );
                if let Expression::StringLiteral(literal) = &member.expression {
                    if literal.value == "length" {
                        items = true;
                    }
                    if is_event_member(&literal.value) {
                        event = true;
                    }
                    if matches!(literal.value.as_str(), "message" | "stack") {
                        error = true;
                    }
                    if member_call {
                        only_property_reads = false;
                        if literal.value == "push" {
                            items = true;
                        } else if is_string_method(&literal.value) {
                            string_ops = true;
                        }
                    }
                } else if member_call {
                    only_property_reads = false;
                }
            }
            AstKind::BinaryExpression(binary) => {
                only_property_reads = false;
                if binary.operator == BinaryOperator::Addition
                    && (is_string_operand(&binary.right) || is_string_operand(&binary.left))
                {
                    string_ops = true;
                }
                if is_arithmetic(binary.operator) || is_comparison(binary.operator) {
                    arithmetic = true;
                }
            }
            AstKind::TemplateLiteral(_) => {
                only_property_reads = false;
                string_ops = true;
            }
            AstKind::UnaryExpression(unary) => {
                only_property_reads = false;
                if matches!(
                    unary.operator,
                    UnaryOperator::UnaryPlus
                        | UnaryOperator::UnaryNegation
                        | UnaryOperator::BitwiseNot
                ) {
                    arithmetic = true;
                }
            }
            _ => {
                only_property_reads = false;
            }
        }
    }

    if called {
        return ("callback".into(), "called as a function".into());
    }
    if event || event_arg {
        return ("event".into(), "reads event fields or is an event listener".into());
    }
    if error {
        return ("error".into(), "reads error fields".into());
    }
    if item_arg {
        return ("item".into(), "element callback of a collection method".into());
    }
    if callback_arg {
        return ("callback".into(), "callback argument of a scheduling or promise call".into());
    }
    if items {
        return (
            "items".into(),
            "used as a length-bearing or indexed collection".into(),
        );
    }
    if only_property_reads && any {
        return ("record".into(), "read through properties only".into());
    }
    if string_ops {
        return ("text".into(), "used in string operations".into());
    }
    if arithmetic {
        return ("value".into(), "used in arithmetic or comparisons".into());
    }
    ("value".into(), "no stronger parameter evidence".into())
}

/// Function names that receive callbacks as parameters.
fn callback_owner(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    [
        "callback",
        "handler",
        "listener",
        "notifier",
        "routine",
        "predicate",
        "comparator",
        "reducer",
        "operator",
        "executor",
        "teardown",
        "subscriber",
        "observer",
        "mapper",
    ]
    .iter()
    .any(|word| lower.contains(word))
}

/// The nearest named function around `start`, for an arrow's parameter roles.
fn enclosing_function_name(nodes: &AstNodes, start: NodeId) -> Option<String> {
    let mut node = start;
    for _ in 0..256 {
        match nodes.kind(node) {
            AstKind::Function(function) => {
                return function.id.as_ref().map(|id| id.name.to_string());
            }
            _ => {}
        }
        let parent = nodes.parent_id(node);
        if parent == node {
            return None;
        }
        node = parent;
    }
    None
}

fn callee_name<'a>(callee: &'a Expression<'a>) -> Option<&'a str> {
    match callee.without_parentheses() {
        Expression::StaticMemberExpression(member) => Some(member.property.name.as_str()),
        Expression::ComputedMemberExpression(member) => match &member.expression {
            Expression::StringLiteral(literal) => Some(literal.value.as_str()),
            _ => None,
        },
        Expression::Identifier(identifier) => Some(identifier.name.as_str()),
        _ => None,
    }
}

fn is_event_member(property: &str) -> bool {
    matches!(
        property,
        "preventDefault"
            | "stopPropagation"
            | "stopImmediatePropagation"
            | "target"
            | "currentTarget"
            | "type"
            | "keyCode"
            | "key"
            | "clientX"
            | "clientY"
            | "deltaY"
            | "touches"
            | "changedTouches"
    )
}

fn is_collection_method(name: &str) -> bool {
    matches!(
        name,
        "push" | "unshift" | "pop" | "shift" | "splice" | "slice" | "concat"
    )
}

fn is_string_method(name: &str) -> bool {
    matches!(
        name,
        "charAt"
            | "charCodeAt"
            | "codePointAt"
            | "at"
            | "indexOf"
            | "lastIndexOf"
            | "includes"
            | "startsWith"
            | "endsWith"
            | "slice"
            | "substring"
            | "substr"
            | "split"
            | "replace"
            | "replaceAll"
            | "match"
            | "matchAll"
            | "toUpperCase"
            | "toLowerCase"
            | "trim"
            | "trimStart"
            | "trimEnd"
            | "concat"
            | "padStart"
            | "padEnd"
            | "repeat"
            | "normalize"
            | "localeCompare"
            | "toString"
    )
}

fn is_numeric_method(name: &str) -> bool {
    matches!(
        name,
        "toFixed" | "toPrecision" | "toExponential" | "valueOf"
    )
}

fn is_numeric(expression: &Expression) -> bool {
    match expression {
        Expression::NumericLiteral(_) | Expression::BigIntLiteral(_) => true,
        Expression::UnaryExpression(unary) => {
            matches!(
                unary.operator,
                UnaryOperator::UnaryPlus | UnaryOperator::UnaryNegation
            ) && is_numeric(&unary.argument)
        }
        _ => false,
    }
}

fn is_string_operand(expression: &Expression) -> bool {
    matches!(
        expression,
        Expression::StringLiteral(_) | Expression::TemplateLiteral(_)
    )
}

fn is_arithmetic(operator: BinaryOperator) -> bool {
    matches!(
        operator,
        BinaryOperator::Addition
            | BinaryOperator::Subtraction
            | BinaryOperator::Multiplication
            | BinaryOperator::Division
            | BinaryOperator::Remainder
            | BinaryOperator::Exponential
            | BinaryOperator::ShiftLeft
            | BinaryOperator::ShiftRight
            | BinaryOperator::ShiftRightZeroFill
            | BinaryOperator::BitwiseOR
            | BinaryOperator::BitwiseXOR
            | BinaryOperator::BitwiseAnd
    )
}

fn is_comparison(operator: BinaryOperator) -> bool {
    matches!(
        operator,
        BinaryOperator::LessThan
            | BinaryOperator::LessEqualThan
            | BinaryOperator::GreaterThan
            | BinaryOperator::GreaterEqualThan
            | BinaryOperator::In
            | BinaryOperator::Instanceof
    )
}

fn equality(operator: BinaryOperator) -> bool {
    matches!(
        operator,
        BinaryOperator::Equality
            | BinaryOperator::Inequality
            | BinaryOperator::StrictEquality
            | BinaryOperator::StrictInequality
    )
}

/// Function/arrow-valued bindings are `handler` when called and `callback`
/// otherwise; the base is otherwise the shape's.
fn assign_names(
    candidates: &[Candidate],
    scoping: &Scoping,
    nodes: &AstNodes,
    program: &Program,
) -> Vec<Option<String>> {
    let refs = reference_nodes(program, scoping);
    let existing = names_by_scope(scoping);
    let unresolved = unresolved_names(program, scoping);
    let regions: BTreeMap<ScopeId, Span> = candidates
        .iter()
        .map(|candidate| (candidate.scope, scope_region(scoping, nodes, candidate.scope)))
        .collect();

    let mut assigned: BTreeMap<String, Vec<(SymbolId, ScopeId)>> = BTreeMap::new();
    let mut chosen_by_symbol: BTreeMap<SymbolId, String> = BTreeMap::new();
    let mut names = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        if candidate.refusal.is_some() {
            names.push(None);
            continue;
        }
        let symbol_refs = refs.get(&candidate.symbol).map(Vec::as_slice).unwrap_or(&[]);
        let mut base = candidate_base(candidate, nodes, symbol_refs);
        // A copy of a binding that already has a readable name inherits it:
        // `let copy = canvas` is a canvas, not a generic value.
        if is_weak_base(&base) {
            if let Some(from) = candidate.from {
                if let Some(name) = chosen_by_symbol.get(&from) {
                    base = name.clone();
                }
            }
        }
        let region = regions
            .get(&candidate.scope)
            .copied()
            .unwrap_or(candidate.declaration_span);
        let mut chosen = None;
        for name in name_attempts(candidate, &base) {
            if name == candidate.original {
                continue;
            }
            if !is_valid_name(&name) {
                continue;
            }
            if name_is_safe(
                candidate,
                &name,
                region,
                scoping,
                &existing,
                &assigned,
                &unresolved,
            ) {
                chosen = Some(name);
                break;
            }
        }
        if let Some(name) = &chosen {
            chosen_by_symbol.insert(candidate.symbol, name.clone());
            assigned
                .entry(name.clone())
                .or_default()
                .push((candidate.symbol, candidate.scope));
        }
        names.push(chosen);
    }
    names
}

fn candidate_base(candidate: &Candidate, nodes: &AstNodes, refs: &[NodeId]) -> String {
    // `evidence` is `"<base>: <explanation>"`, except for the explicit kinds
    // whose base is fixed.
    if candidate.kind == LocalBindingKind::CatchParameter {
        return "error".to_string();
    }
    if candidate.kind == LocalBindingKind::LoopCounter {
        return counter_name(candidate.counter_depth);
    }
    if let Some((base, _)) = candidate.evidence.split_once(": ") {
        if (base == "callback" || base == "handler") && !refs.is_empty() && is_called(nodes, refs) {
            return "handler".to_string();
        }
        return base.to_string();
    }
    "value".to_string()
}

/// A readable class base from the factory that creates it:
/// `createValueEncoderClass` -> `ValueEncoder`.
fn class_base_from_factory(name: &str) -> String {
    let base = name.strip_prefix("create").unwrap_or(name);
    let base = base.strip_suffix("Class").unwrap_or(base).trim();
    // `createRegistryClassWithLazyHookInstall` names the class, not the
    // prefix: drop the internal `Class` token as well.
    let base = base.replace("Class", "");
    let base = base.trim();
    if base.is_empty() {
        return "Anonymous".to_string();
    }
    let mut chars = base.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => "Anonymous".to_string(),
    }
}

fn counter_name(index: usize) -> String {
    const NAMES: [&str; 8] = ["i", "j", "k", "l", "m", "n", "p", "q"];
    NAMES
        .get(index)
        .map(|name| (*name).to_string())
        .unwrap_or_else(|| format!("index{}", index + 1))
}

fn name_attempts(candidate: &Candidate, base: &str) -> Vec<String> {
    let mut names = Vec::new();
    if candidate.kind == LocalBindingKind::ClassIdentity {
        names.push(base.to_string());
        names.push(format!("{base}Class"));
        for suffix in 2..=64 {
            names.push(format!("{base}Class{suffix}"));
        }
        return names;
    }
    if candidate.kind == LocalBindingKind::ClassIdentity {
        names.push(base.to_string());
        names.push(format!("{base}Class"));
        for suffix in 2..=64 {
            names.push(format!("{base}Class{suffix}"));
        }
        return names;
    }
    if candidate.kind == LocalBindingKind::LoopCounter {
        let start = candidate.counter_depth;
        for index in start..start + 8 {
            names.push(counter_name(index));
        }
        for suffix in 2..=64 {
            names.push(format!("{base}{suffix}"));
        }
    } else {
        names.push(base.to_string());
        // Large generated functions can hold hundreds of locals with the same
        // kind evidence; the suffix budget must cover the scope, or the tail
        // would keep a generated spelling.
        for suffix in 2..=1024 {
            names.push(format!("{base}{suffix}"));
        }
    }
    names
}

/// Is `name` free of shadowing/capture conflicts for `candidate`?
fn name_is_safe(
    candidate: &Candidate,
    name: &str,
    region: Span,
    scoping: &Scoping,
    existing: &BTreeMap<String, Vec<(SymbolId, ScopeId)>>,
    assigned: &BTreeMap<String, Vec<(SymbolId, ScopeId)>>,
    unresolved: &BTreeMap<String, Vec<Span>>,
) -> bool {
    for (symbol, scope) in existing.get(name).into_iter().flatten() {
        if *symbol == candidate.symbol {
            continue;
        }
        if scopes_related(scoping, candidate.scope, *scope) {
            return false;
        }
    }
    for (symbol, scope) in assigned.get(name).into_iter().flatten() {
        if *symbol == candidate.symbol {
            continue;
        }
        if scopes_related(scoping, candidate.scope, *scope) {
            return false;
        }
    }
    if unresolved
        .get(name)
        .is_some_and(|spans| spans.iter().any(|span| contains(region, *span)))
    {
        return false;
    }
    true
}

fn names_by_scope(scoping: &Scoping) -> BTreeMap<String, Vec<(SymbolId, ScopeId)>> {
    let mut names: BTreeMap<String, Vec<(SymbolId, ScopeId)>> = BTreeMap::new();
    for symbol in scoping.symbol_ids() {
        names
            .entry(scoping.symbol_name(symbol).to_string())
            .or_default()
            .push((symbol, scoping.symbol_scope_id(symbol)));
    }
    names
}

fn scopes_related(scoping: &Scoping, left: ScopeId, right: ScopeId) -> bool {
    scoping.scope_ancestors(left).any(|scope| scope == right)
        || scoping.scope_ancestors(right).any(|scope| scope == left)
}

fn scope_region(scoping: &Scoping, nodes: &AstNodes, scope: ScopeId) -> Span {
    nodes.kind(scoping.get_node_id(scope)).span()
}

fn unresolved_names(program: &Program, scoping: &Scoping) -> BTreeMap<String, Vec<Span>> {
    struct Collect<'s> {
        scoping: &'s Scoping,
        names: BTreeMap<String, Vec<Span>>,
    }
    impl<'s, 'a> Visit<'a> for Collect<'s> {
        fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
            if analysis::identifier_symbol(identifier, self.scoping).is_none() {
                self.names
                    .entry(identifier.name.to_string())
                    .or_default()
                    .push(identifier.span);
            }
        }
    }
    let mut collect = Collect {
        scoping,
        names: BTreeMap::new(),
    };
    collect.visit_program(program);
    collect.names
}

fn reference_nodes(program: &Program, scoping: &Scoping) -> BTreeMap<SymbolId, Vec<NodeId>> {
    struct Collect<'s> {
        scoping: &'s Scoping,
        refs: BTreeMap<SymbolId, Vec<NodeId>>,
    }
    impl<'s, 'a> Visit<'a> for Collect<'s> {
        fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
            if let Some(symbol) = analysis::identifier_symbol(identifier, self.scoping) {
                self.refs
                    .entry(symbol)
                    .or_default()
                    .push(identifier.node_id.get());
            }
        }
    }
    let mut collect = Collect {
        scoping,
        refs: BTreeMap::new(),
    };
    collect.visit_program(program);
    collect.refs
}

fn is_called(nodes: &AstNodes, refs: &[NodeId]) -> bool {
    refs.iter().any(|node| {
        let span = nodes.kind(*node).span();
        let parent = nodes.parent_id(*node);
        match nodes.kind(parent) {
            AstKind::CallExpression(call) => call.callee.span() == span,
            AstKind::StaticMemberExpression(member) if member.object.span() == span => {
                let call_like = matches!(member.property.name.as_str(), "call" | "apply" | "bind");
                call_like
                    && matches!(
                        nodes.parent_kind(parent),
                        AstKind::CallExpression(call)
                            if call.callee.span() == nodes.kind(parent).span()
                    )
            }
            _ => false,
        }
    })
}

/// The spans of every function containing a direct `eval` reference or a
/// `with` statement in its own body (nested functions are separate).
fn dynamic_functions(program: &Program) -> BTreeSet<Span> {
    struct Dynamic {
        stack: Vec<Span>,
        found: BTreeSet<Span>,
    }
    impl Dynamic {
        fn mark(&mut self) {
            if let Some(span) = self.stack.last() {
                self.found.insert(*span);
            }
        }
    }
    impl<'a> Visit<'a> for Dynamic {
        fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
            self.stack.push(function.span);
            walk::walk_function(self, function, flags);
            self.stack.pop();
        }

        fn visit_arrow_function_expression(&mut self, arrow: &ArrowFunctionExpression<'a>) {
            self.stack.push(arrow.span);
            walk::walk_arrow_function_expression(self, arrow);
            self.stack.pop();
        }

        fn visit_with_statement(&mut self, statement: &WithStatement<'a>) {
            self.mark();
            walk::walk_with_statement(self, statement);
        }

        fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
            if identifier.name == "eval" {
                self.mark();
            }
        }
    }
    let mut dynamic = Dynamic {
        stack: Vec::new(),
        found: BTreeSet::new(),
    };
    dynamic.visit_program(program);
    dynamic.found
}

// ---------------------------------------------------------------------------
// Post-edit validation
// ---------------------------------------------------------------------------

struct EditInfo {
    start: u32,
    end: u32,
    new_len: u32,
}

/// Rewrite an offset in the edited text back to the input coordinate space.
fn unshift(offset: u32, edits: &[EditInfo]) -> u32 {
    let mut delta: i64 = 0;
    for edit in edits {
        let out_start = edit.start as i64 + delta;
        let out_end = out_start + edit.new_len as i64;
        let offset = offset as i64;
        if offset < out_start {
            break;
        }
        if offset < out_end {
            return edit.start;
        }
        delta += edit.new_len as i64 - (edit.end - edit.start) as i64;
    }
    let original = offset as i64 - delta;
    original.max(0) as u32
}

struct ReferenceRecord {
    start: u32,
    name: String,
    target: Option<u32>,
}

fn reference_records(program: &Program, scoping: &Scoping) -> Vec<ReferenceRecord> {
    struct Collect<'s> {
        scoping: &'s Scoping,
        out: Vec<ReferenceRecord>,
    }
    impl<'s, 'a> Visit<'a> for Collect<'s> {
        fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
            let symbol = analysis::identifier_symbol(identifier, self.scoping);
            self.out.push(ReferenceRecord {
                start: identifier.span.start,
                name: identifier.name.to_string(),
                target: symbol.map(|symbol| self.scoping.symbol_span(symbol).start),
            });
        }
    }
    let mut collect = Collect {
        scoping,
        out: Vec::new(),
    };
    collect.visit_program(program);
    collect.out
}

fn free_names(records: &[ReferenceRecord]) -> Vec<String> {
    let mut names: Vec<String> = records
        .iter()
        .filter(|record| record.target.is_none())
        .map(|record| record.name.clone())
        .collect();
    names.sort();
    names
}

/// The fail-closed post-edit check: no parse/early error, identical reference
/// resolution, identical free names, and every renamed binding actually carries
/// its new name.
///
/// Returns the number of generated local bindings still present in the output.
fn validate_rewrite(
    source_refs: &[ReferenceRecord],
    rewritten: &str,
    edits: &[EditInfo],
    renamed_targets: &BTreeMap<u32, String>,
) -> Result<usize, String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, rewritten)
        .map_err(|error| format!("the rewritten source did not parse: {error}"))?;
    let errors: Vec<String> = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&program)
        .errors
        .iter()
        .map(ToString::to_string)
        .collect();
    if !errors.is_empty() {
        return Err(format!(
            "the rewritten source has early errors: {}",
            errors.join("; ")
        ));
    }
    let (scoping, nodes) = analysis::analyze(&program);
    let output_refs = reference_records(&program, &scoping);

    // Every reference must still resolve to the same declaration (or stay
    // free). This is the capture proof: a rename that shadowed or captured
    // another binding would change one entry.
    let mut before: Vec<(u32, Option<u32>)> = source_refs
        .iter()
        .map(|record| (record.start, record.target))
        .collect();
    let mut after: Vec<(u32, Option<u32>)> = output_refs
        .iter()
        .map(|record| {
            (
                unshift(record.start, edits),
                record.target.map(|target| unshift(target, edits)),
            )
        })
        .collect();
    before.sort_by_key(|(start, target)| (*start, *target));
    after.sort_by_key(|(start, target)| (*start, *target));
    if before != after {
        let first = before
            .iter()
            .zip(after.iter())
            .position(|(a, b)| a != b)
            .map(|at| (at, before.get(at), after.get(at)));
        return Err(format!(
            "reference resolution changed; a local rename would capture or lose a binding (first diff: {first:?}, before {} after {})",
            before.len(),
            after.len()
        ));
    }

    if free_names(source_refs) != free_names(&output_refs) {
        return Err("the free-name set changed; a local rename would capture a free name".into());
    }

    // Every renamed declaration and reference must actually spell the new
    // name; a stale generated reference is never accepted.
    for record in &output_refs {
        let original_start = unshift(record.start, edits);
        let original_target = record.target.map(|target| unshift(target, edits));
        if let Some(name) = original_target.and_then(|target| renamed_targets.get(&target)) {
            if record.name != *name {
                return Err(format!(
                    "a reference to a renamed binding at byte {original_start} still spells `{}`",
                    record.name
                ));
            }
        }
    }
    let sites = collect_binding_identifiers(&program);
    for binding in &sites {
        let original_start = unshift(binding.span.start, edits);
        if let Some(name) = renamed_targets.get(&original_start) {
            let current = scoping.symbol_name(binding.symbol);
            if current != name {
                return Err(format!(
                    "a renamed binding at byte {original_start} still spells `{current}`"
                ));
            }
        }
    }

    Ok(collect_candidates(&program, &scoping, &nodes, &sites).len())
}

fn contains(outer: Span, inner: Span) -> bool {
    inner.start >= outer.start && inner.end <= outer.end
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(source: &str) -> LocalNamesResult {
        apply_local_names(source, LocalNameOptions::default()).expect("source parses")
    }

    fn name_of<'a>(result: &'a LocalNamesResult, original: &str) -> Option<&'a str> {
        result
            .outcomes
            .iter()
            .find(|outcome| outcome.original_name == original)
            .and_then(|outcome| outcome.new_name.as_deref())
    }

    #[test]
    fn catch_parameter_is_error_and_loop_counter_is_i() {
        let result = run("function f(){ try {} catch ($E) {} for (let $K = 0; $K < 3; $K++) {} } f();");
        assert_eq!(name_of(&result, "$E"), Some("error"));
        assert_eq!(name_of(&result, "$K"), Some("i"));
        assert_eq!(result.renamed, 2);
        assert_eq!(result.remaining, 0);
    }

    #[test]
    fn forwarding_copy_is_value() {
        let result = run("function f($A){ let $B = $A; return $B; } f(1);");
        assert!(
            name_of(&result, "$B").is_some_and(|name| name.starts_with("value")),
            "{:?}",
            result.outcomes
        );
    }

    #[test]
    fn disabled_is_byte_identical() {
        let source = "function f($A){ let $B = 1; return $B + $A; } f(1);";
        let result = apply_local_names(source, LocalNameOptions::disabled()).expect("parses");
        assert_eq!(result.transformed_source, source);
        assert_eq!(result.remaining, 2);
        assert_eq!(result.outcomes.len(), 0);
    }
    #[test]
    fn class_identity_is_named_after_its_factory() {
        let result = run(
            "function createThingClass(){ class $C { m(){ return 1; } } return $C; } globalThis.x = createThingClass();",
        );
        assert_eq!(name_of(&result, "$C"), Some("Thing"));
        assert!(result.transformed_source.contains("class Thing"));
    }

    #[test]
    fn weak_initializer_takes_the_shape_of_its_writes() {
        let result = run(
            "function f($C){ let $E = void 0; $E = $C.createElement(\"canvas\"); return $E; } f(document);",
        );
        assert_eq!(name_of(&result, "$E"), Some("element"));
    }

    #[test]
    fn compound_assignment_is_an_accumulator() {
        let result = run(
            "function f($X){ let $H = 0; for (let $I = 0; $I < 2; $I++) { $H |= $X[$I]; } return $H; } f([1]);",
        );
        assert_eq!(name_of(&result, "$H"), Some("hash"));
        assert_eq!(name_of(&result, "$I"), Some("i"));
    }

    #[test]
    fn call_through_call_is_called() {
        let result = run(
            "function f($C){ $C.call(void 0, 1); } f(function(){});",
        );
        assert_eq!(name_of(&result, "$C"), Some("handler"));
    }

}
