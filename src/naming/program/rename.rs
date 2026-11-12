//! Lexical-symbol binding renames.
//!
//! Given a binding identifier's byte offset, the planner resolves the owning
//! Oxc symbol, checks the rename is safe, and returns exact source edits for
//! the declaration and every reference that resolves to it — including
//! references that appear before the declaration, and shadowed bindings in
//! nested scopes.

use std::collections::BTreeSet;

use oxc_ast::ast::{BindingIdentifier, IdentifierReference, Program};
use oxc_ast::AstKind;
use oxc_ast_visit::Visit;
use oxc_semantic::{AstNodes, NodeId, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::scope::ScopeId;

use crate::analysis;

use super::{DiagnosticCode, ProgramBindingRename, ProgramTransformDiagnostic, SpanRange};

/// The outcome of one rename plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameOutcome {
    /// The requested declaration offset.
    pub declaration_start: u32,
    /// The binding's name before the rename, once resolved.
    pub original_name: Option<String>,
    /// The requested replacement name.
    pub canonical_name: String,
    /// Whether the rename was applied.
    pub applied: bool,
    /// The reason it was skipped, when `applied` is `false`.
    pub skip_reason: Option<RenameSkipReason>,
    /// The declaration identifier's span, once resolved.
    pub declaration_span: Option<SpanRange>,
    /// How many reference sites were rewritten.
    pub reference_edits: usize,
    /// How many sites were expanded from a shorthand (`{ x }` -> `{ x: name }`).
    pub shorthand_edits: usize,
}

/// Why a rename plan was not applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenameSkipReason {
    /// Renames are disabled by the options.
    Disabled,
    /// No binding identifier begins at the requested offset.
    NoDeclarationAtOffset,
    /// The replacement name is not a valid, non-reserved identifier.
    InvalidCanonicalName,
    /// The replacement name equals the current name.
    Unchanged,
    /// The replacement name would be captured by, or would capture, another name.
    NameCollision,
    /// A binding site cannot be rewritten exactly; the whole plan is skipped.
    UnsupportedBindingSite(String),
    /// The program uses a dynamic scope (`with`/`eval`).
    DynamicScope,
    /// Another plan already targeted the same declaration.
    DuplicateTarget,
    /// Another plan in the same batch would give a different binding the same
    /// name where the two would collide; both plans are skipped.
    SimultaneousCollision,
    /// The rewrite was abandoned because an edit overlapped.
    OverlappingEdit,
    /// The whole rewrite was abandoned because the rewritten text failed the
    /// post-edit validation: it did not reparse, or it has a semantic early
    /// error (for example a redeclaration) the original did not.
    RewriteRejected,
}

pub(super) struct RenamePlan {
    pub edits: Vec<analysis::Edit>,
    pub outcomes: Vec<RenameOutcome>,
    pub diagnostics: Vec<ProgramTransformDiagnostic>,
}

/// A rename that passed every single-plan check and is waiting for the
/// joint (simultaneous) validation that two plans in the same batch cannot
/// give two different bindings the same name where they would collide.
struct PendingRename {
    /// Index of this plan's outcome in [`RenamePlan::outcomes`].
    plan_index: usize,
    /// The resolved lexical symbol being renamed.
    symbol: SymbolId,
    /// The scope the symbol is declared in.
    scope: ScopeId,
    /// Every scope a declaration of the symbol occupies; see
    /// [`declaration_scopes`].
    declaration_scopes: BTreeSet<ScopeId>,
    /// The declaration identifier's span.
    declaration_span: SpanRange,
    /// The binding's name before the rename.
    original_name: String,
    /// The requested replacement name.
    canonical_name: String,
    /// The exact edits this plan would apply.
    edits: Vec<analysis::Edit>,
    /// How many reference sites would be rewritten.
    reference_edits: usize,
    /// How many sites would be expanded from a shorthand.
    shorthand_edits: usize,
}

pub(super) fn plan_renames(
    program: &Program,
    scoping: &Scoping,
    nodes: &AstNodes,
    renames: &[ProgramBindingRename],
    dynamic_scope: bool,
    enabled: bool,
) -> RenamePlan {
    let unresolved = collect_unresolved(program, scoping);
    let sites = collect_binding_identifiers(program);
    let script = !program.source_type.is_module();
    let mut plan = RenamePlan {
        edits: Vec::new(),
        outcomes: Vec::new(),
        diagnostics: Vec::new(),
    };
    let mut seen_targets: BTreeSet<u32> = BTreeSet::new();
    let mut pending: Vec<PendingRename> = Vec::new();

    for rename in renames {
        let mut outcome = RenameOutcome {
            declaration_start: rename.declaration_start,
            original_name: None,
            canonical_name: rename.canonical_name.clone(),
            applied: false,
            skip_reason: None,
            declaration_span: None,
            reference_edits: 0,
            shorthand_edits: 0,
        };

        if !enabled {
            outcome.skip_reason = Some(RenameSkipReason::Disabled);
            plan.outcomes.push(outcome);
            continue;
        }

        if !seen_targets.insert(rename.declaration_start) {
            outcome.skip_reason = Some(RenameSkipReason::DuplicateTarget);
            plan.diagnostics.push(diagnostic(
                DiagnosticCode::DuplicateTarget,
                format!(
                    "two rename plans target the declaration at byte {}",
                    rename.declaration_start
                ),
                None,
            ));
            plan.outcomes.push(outcome);
            continue;
        }

        let Some(symbol) = symbol_at(&sites, rename.declaration_start) else {
            outcome.skip_reason = Some(RenameSkipReason::NoDeclarationAtOffset);
            plan.diagnostics.push(diagnostic(
                DiagnosticCode::UnresolvedDeclaration,
                format!(
                    "no binding identifier begins at byte {}",
                    rename.declaration_start
                ),
                None,
            ));
            plan.outcomes.push(outcome);
            continue;
        };

        let original = scoping.symbol_name(symbol).to_owned();
        let declaration_span = span_range(scoping.symbol_span(symbol));
        outcome.original_name = Some(original.clone());
        outcome.declaration_span = Some(declaration_span);

        if dynamic_scope {
            outcome.skip_reason = Some(RenameSkipReason::DynamicScope);
            plan.diagnostics.push(diagnostic(
                DiagnosticCode::DynamicScope,
                format!(
                    "cannot rename `{original}`: the program contains a dynamic scope (with/eval)"
                ),
                Some(declaration_span),
            ));
            plan.outcomes.push(outcome);
            continue;
        }

        if original == rename.canonical_name {
            outcome.skip_reason = Some(RenameSkipReason::Unchanged);
            plan.outcomes.push(outcome);
            continue;
        }

        if !is_valid_name(&rename.canonical_name) {
            outcome.skip_reason = Some(RenameSkipReason::InvalidCanonicalName);
            plan.diagnostics.push(diagnostic(
                DiagnosticCode::InvalidCanonicalName,
                format!(
                    "`{}` is not a usable identifier name",
                    rename.canonical_name
                ),
                Some(declaration_span),
            ));
            plan.outcomes.push(outcome);
            continue;
        }

        if let Some(message) = collision(
            scoping,
            nodes,
            &sites,
            symbol,
            &rename.canonical_name,
            &unresolved,
            script,
        ) {
            outcome.skip_reason = Some(RenameSkipReason::NameCollision);
            plan.diagnostics.push(diagnostic(
                DiagnosticCode::NameCollision,
                message,
                Some(declaration_span),
            ));
            plan.outcomes.push(outcome);
            continue;
        }

        if let Some(span) = annex_b_outer_reference(scoping, nodes, symbol, &original, &unresolved)
        {
            let reason = "a block-level function is also used through its sloppy-mode \
                          function-scope binding, which does not resolve to this declaration";
            outcome.skip_reason = Some(RenameSkipReason::UnsupportedBindingSite(reason.into()));
            plan.diagnostics.push(diagnostic(
                DiagnosticCode::UnsupportedBindingSite,
                format!("cannot rename `{original}`: {reason}"),
                Some(span_range(span)),
            ));
            plan.outcomes.push(outcome);
            continue;
        }

        match collect_sites(
            scoping,
            nodes,
            &sites,
            symbol,
            &original,
            &rename.canonical_name,
        ) {
            Ok((edits, references, shorthands)) => {
                // Defer marking the plan applied until the whole batch has
                // been checked for simultaneous collisions.
                pending.push(PendingRename {
                    plan_index: plan.outcomes.len(),
                    symbol,
                    scope: scoping.symbol_scope_id(symbol),
                    declaration_scopes: declaration_scopes(scoping, nodes, &sites, symbol),
                    declaration_span,
                    original_name: original.clone(),
                    canonical_name: rename.canonical_name.clone(),
                    edits,
                    reference_edits: references,
                    shorthand_edits: shorthands,
                });
            }
            Err((reason, site)) => {
                outcome.skip_reason = Some(RenameSkipReason::UnsupportedBindingSite(reason.into()));
                plan.diagnostics.push(diagnostic(
                    DiagnosticCode::UnsupportedBindingSite,
                    format!("cannot rename `{original}`: {reason}"),
                    Some(span_range(site)),
                ));
            }
        }
        plan.outcomes.push(outcome);
    }

    resolve_simultaneous_collisions(scoping, nodes, &pending, &mut plan);
    plan
}

/// Validate the pending plans against each other and apply the survivors.
///
/// Each pending plan already passed the per-plan checks, which compare a
/// proposed name against the *original* bindings only. They cannot see that
/// two different bindings in the same batch are being given the same fresh
/// name: two `var`s in one scope renamed to `shared` still parse as
/// `var shared; var shared`, but they collapse into one binding; and a
/// parent/child pair renamed to the same name can capture a reference the
/// original program resolved to the outer binding. When two plans would
/// collide, both are skipped with a typed diagnostic rather than applied,
/// so the batch never changes semantics silently.
///
/// Plans in distinct sibling scopes that share a canonical name do not
/// collide and both apply.
fn resolve_simultaneous_collisions(
    scoping: &Scoping,
    nodes: &AstNodes,
    pending: &[PendingRename],
    plan: &mut RenamePlan,
) {
    let mut blocked: BTreeSet<usize> = BTreeSet::new();
    for first in 0..pending.len() {
        for second in (first + 1)..pending.len() {
            if pending[first].canonical_name != pending[second].canonical_name {
                continue;
            }
            if simultaneous_collision(scoping, nodes, &pending[first], &pending[second]) {
                blocked.insert(first);
                blocked.insert(second);
            }
        }
    }

    for (index, rename) in pending.iter().enumerate() {
        let outcome = &mut plan.outcomes[rename.plan_index];
        if blocked.contains(&index) {
            outcome.applied = false;
            outcome.skip_reason = Some(RenameSkipReason::SimultaneousCollision);
            plan.diagnostics.push(diagnostic(
                DiagnosticCode::SimultaneousCollision,
                format!(
                    "renaming `{}` to `{}` collides with another plan that gives `{}` to a \
                     different binding in the same batch",
                    rename.original_name, rename.canonical_name, rename.canonical_name
                ),
                Some(rename.declaration_span),
            ));
        } else {
            outcome.applied = true;
            outcome.reference_edits = rename.reference_edits;
            outcome.shorthand_edits = rename.shorthand_edits;
            plan.edits.extend(rename.edits.iter().cloned());
        }
    }
}

/// The scope facts one pending rename needs for the simultaneous-collision
/// check, exposed to upstream planners so they can pre-allocate distinct
/// concrete names without duplicating the exact validator.
#[derive(Clone, Debug)]
pub(crate) struct PlannedBindingFacts {
    pub(crate) symbol: SymbolId,
    pub(crate) scope: ScopeId,
    pub(crate) declaration_scopes: BTreeSet<ScopeId>,
    /// The requested base name would already collide for this binding alone
    /// (the per-plan `collision` check), so the old batch never contained it.
    pub(crate) name_conflicts: bool,
    /// The binding is a sloppy block-level declaration with an unresolved
    /// Annex B outer use; its plan is refused whatever the requested name.
    pub(crate) annex_b_excluded: bool,
}

/// Collect the facts for exactly the declarations a planner is considering.
///
/// The upstream thread namer needs to know whether two *different* generated
/// bindings would collide once they share a requested concrete name, and which
/// bindings the older planner would have removed from the pending batch for a
/// name-independent or per-plan reason. It gathers the facts once per
/// candidate group here instead of re-parsing or re-deriving the scope rules in
/// a second place. A missing declaration offset stays `None` so the caller can
/// fall back to the validator's own refusal.
pub(crate) fn planned_binding_facts_for(
    program: &Program,
    scoping: &Scoping,
    nodes: &AstNodes,
    declaration_starts: &[u32],
    requested_name: &str,
) -> Vec<Option<PlannedBindingFacts>> {
    let sites = collect_binding_identifiers(program);
    let unresolved = collect_unresolved(program, scoping);
    let script = !program.source_type.is_module();
    declaration_starts
        .iter()
        .map(|&start| -> Option<PlannedBindingFacts> {
            let symbol = symbol_at(&sites, start)?;
            let original = scoping.symbol_name(symbol);
            Some(PlannedBindingFacts {
                symbol,
                scope: scoping.symbol_scope_id(symbol),
                declaration_scopes: declaration_scopes(scoping, nodes, &sites, symbol),
                name_conflicts: collision(
                    scoping,
                    nodes,
                    &sites,
                    symbol,
                    requested_name,
                    &unresolved,
                    script,
                )
                .is_some(),
                annex_b_excluded: annex_b_outer_reference(
                    scoping,
                    nodes,
                    symbol,
                    original,
                    &unresolved,
                )
                .is_some(),
            })
        })
        .collect()
}

/// The exact simultaneous-collision relation: would giving `first` and
/// `second` the *same* concrete name merge or capture one of them?
///
/// This is the same predicate [`resolve_simultaneous_collisions`] applies to a
/// pending batch. It is exposed so the upstream thread namer can hand the
/// validator a set of already-distinct names instead of guessing.
pub(crate) fn planned_bindings_collide(
    scoping: &Scoping,
    nodes: &AstNodes,
    first: &PlannedBindingFacts,
    second: &PlannedBindingFacts,
) -> bool {
    if first.symbol == second.symbol {
        return false;
    }
    if !first
        .declaration_scopes
        .is_disjoint(&second.declaration_scopes)
    {
        return true;
    }
    if is_ancestor_scope(scoping, first.scope, second.scope) {
        return references_within_scope(scoping, nodes, first.symbol, second.scope);
    }
    if is_ancestor_scope(scoping, second.scope, first.scope) {
        return references_within_scope(scoping, nodes, second.symbol, first.scope);
    }
    false
}

/// Would applying both plans give two different bindings the same canonical
/// name where a reference's resolution would change?
///
/// Two distinct symbols that end up with the same name collide when their
/// declarations share a scope (see [`declaration_scopes`]: a hoisted `var`
/// also occupies every block it hoists through), or when one is declared in a
/// scope enclosing the other *and* the outer binding has a reference inside
/// the inner scope (that reference would be captured by the inner, now
/// same-named, binding). Scopes that are neither shared nor nested are
/// siblings: their names never see each other, so they are safe.
fn simultaneous_collision(
    scoping: &Scoping,
    nodes: &AstNodes,
    first: &PendingRename,
    second: &PendingRename,
) -> bool {
    if first.symbol == second.symbol {
        return false;
    }
    if !first
        .declaration_scopes
        .is_disjoint(&second.declaration_scopes)
    {
        return true;
    }
    if is_ancestor_scope(scoping, first.scope, second.scope) {
        return references_within_scope(scoping, nodes, first.symbol, second.scope);
    }
    if is_ancestor_scope(scoping, second.scope, first.scope) {
        return references_within_scope(scoping, nodes, second.symbol, first.scope);
    }
    false
}

/// Is `ancestor` a strict ancestor of `scope`? [`Scoping::scope_ancestors`]
/// yields the scope itself first, so the strict test is just membership.
fn is_ancestor_scope(scoping: &Scoping, ancestor: ScopeId, scope: ScopeId) -> bool {
    scoping
        .scope_ancestors(scope)
        .any(|candidate| candidate == ancestor)
}

/// Does `symbol` have a resolved reference whose span lies inside `scope`?
fn references_within_scope(
    scoping: &Scoping,
    nodes: &AstNodes,
    symbol: SymbolId,
    scope: ScopeId,
) -> bool {
    let region = scope_span(scoping, nodes, scope);
    scoping
        .get_resolved_references(symbol)
        .any(|reference| contains(region, nodes.kind(reference.node_id()).span()))
}

/// One declared binding identifier: its span, its AST node and its symbol.
pub(super) struct BindingSite {
    pub(super) span: Span,
    pub(super) node: NodeId,
    pub(super) symbol: SymbolId,
}

/// Every `BindingIdentifier` in the program, so a declaration's identifier
/// span and its own node are both available (a `symbol_declaration` node is
/// the enclosing declarator, whose span also covers the initializer).
pub(super) fn collect_binding_identifiers(program: &Program) -> Vec<BindingSite> {
    struct Collect {
        sites: Vec<BindingSite>,
    }
    impl<'a> Visit<'a> for Collect {
        fn visit_binding_identifier(&mut self, identifier: &BindingIdentifier<'a>) {
            if let Some(symbol) = identifier.symbol_id.get() {
                self.sites.push(BindingSite {
                    span: identifier.span,
                    node: identifier.node_id.get(),
                    symbol,
                });
            }
        }
    }
    let mut collect = Collect { sites: Vec::new() };
    collect.visit_program(program);
    collect.sites
}

/// The symbol whose declaration identifier starts at `offset`, if any.
fn symbol_at(sites: &[BindingSite], offset: u32) -> Option<SymbolId> {
    sites
        .iter()
        .find(|site| site.span.start == offset)
        .map(|site| site.symbol)
}

pub(super) fn collect_sites(
    scoping: &Scoping,
    nodes: &AstNodes,
    sites: &[BindingSite],
    symbol: SymbolId,
    original: &str,
    new_name: &str,
) -> Result<(Vec<analysis::Edit>, usize, usize), (&'static str, Span)> {
    let mut edits = Vec::new();
    let mut references = 0usize;
    let mut shorthands = 0usize;

    for site in sites.iter().filter(|site| site.symbol == symbol) {
        let (edit, expanded) = site_edit(nodes, site.node, site.span, original, new_name)
            .map_err(|reason| (reason, site.span))?;
        if expanded {
            shorthands += 1;
        }
        edits.push(edit);
    }

    for reference in scoping.get_resolved_references(symbol) {
        let node = reference.node_id();
        let id_span = nodes.kind(node).span();
        let (edit, expanded) = site_edit(nodes, node, id_span, original, new_name)
            .map_err(|reason| (reason, id_span))?;
        references += 1;
        if expanded {
            shorthands += 1;
        }
        edits.push(edit);
    }

    Ok((edits, references, shorthands))
}

/// Build the edit for one binding site. `Ok((edit, expanded))` where `expanded`
/// marks a shorthand that was widened to `original: new_name`.
fn site_edit(
    nodes: &AstNodes,
    node: NodeId,
    id_span: Span,
    original: &str,
    new_name: &str,
) -> Result<(analysis::Edit, bool), &'static str> {
    if matches!(
        nodes.kind(node),
        AstKind::ImportSpecifier(_)
            | AstKind::ImportDefaultSpecifier(_)
            | AstKind::ImportNamespaceSpecifier(_)
            | AstKind::ExportSpecifier(_)
    ) {
        return Err("an import/export binding cannot be rewritten exactly");
    }
    let shorthand = match nodes.parent_kind(node) {
        AstKind::ObjectProperty(property) if property.shorthand => {
            same_span(property.key.span(), id_span)
        }
        AstKind::BindingProperty(property) if property.shorthand => {
            same_span(property.key.span(), id_span)
        }
        AstKind::AssignmentTargetPropertyIdentifier(_) => true,
        AstKind::AssignmentPattern(_) => {
            let parent = nodes.parent_kind(nodes.parent_id(node));
            match parent {
                AstKind::BindingProperty(property) if property.shorthand => {
                    same_span(property.key.span(), id_span)
                }
                _ => false,
            }
        }
        AstKind::ExportSpecifier(_)
        | AstKind::ImportSpecifier(_)
        | AstKind::ImportDefaultSpecifier(_)
        | AstKind::ImportNamespaceSpecifier(_) => {
            return Err("an import/export binding cannot be rewritten exactly");
        }
        _ => false,
    };
    let replacement = if shorthand {
        format!("{original}: {new_name}")
    } else {
        new_name.to_owned()
    };
    Ok((analysis::Edit::replace(id_span, replacement), shorthand))
}

/// Would renaming `symbol` to `new_name` redeclare, capture, or be captured
/// by a different binding (or a free global) that is live in the same region?
///
/// Any other binding of `new_name` whose declarations share a scope with ours
/// collides whether or not it is referenced: two `var`s (or a `var` and a
/// function or parameter) would silently merge into one binding, and any pair
/// involving `let`/`const`/`class` would be a redeclaration error.
fn collision(
    scoping: &Scoping,
    nodes: &AstNodes,
    sites: &[BindingSite],
    symbol: SymbolId,
    new_name: &str,
    unresolved: &[(Span, String)],
    script: bool,
) -> Option<String> {
    let scope = scoping.symbol_scope_id(symbol);
    let scopes = declaration_scopes(scoping, nodes, sites, symbol);
    let target_span = declaration_region(scoping, nodes, &scopes);

    if script
        && scopes.contains(&scoping.root_scope_id())
        && matches!(new_name, "undefined" | "NaN" | "Infinity")
    {
        // A top-level script binding is a property of the global object, and
        // these three are non-configurable there: a `var` is silently ignored
        // and a lexical declaration throws.
        return Some(format!(
            "renaming to `{new_name}` would redeclare a non-configurable global"
        ));
    }

    for other in scoping.symbol_ids() {
        if other == symbol || scoping.symbol_name(other) != new_name {
            continue;
        }
        if !scopes.is_disjoint(&declaration_scopes(scoping, nodes, sites, other)) {
            return Some(format!(
                "renaming to `{new_name}` would redeclare an existing `{new_name}` in the same scope"
            ));
        }
        let other_scope = scoping.symbol_scope_id(other);
        if scoping
            .scope_ancestors(scope)
            .any(|ancestor| ancestor == other_scope)
        {
            // `new_name` is bound in an enclosing scope: any reference to it
            // inside our scope would be captured by the new binding.
            for reference in scoping.get_resolved_references(other) {
                if contains(target_span, nodes.kind(reference.node_id()).span()) {
                    return Some(format!(
                        "renaming to `{new_name}` would capture an existing `{new_name}` used in this scope"
                    ));
                }
            }
        } else if scoping
            .scope_ancestors(other_scope)
            .any(|ancestor| ancestor == scope)
        {
            // `new_name` is bound in a nested scope, referenced or not: one of
            // our references inside it would be captured by that binding.
            let other_span = scope_span(scoping, nodes, other_scope);
            for reference in scoping.get_resolved_references(symbol) {
                if contains(other_span, nodes.kind(reference.node_id()).span()) {
                    return Some(format!(
                        "renaming to `{new_name}` would be captured by an inner `{new_name}`"
                    ));
                }
            }
        }
    }

    for (span, name) in unresolved {
        if name == new_name && contains(target_span, *span) {
            return Some(format!(
                "renaming to `{new_name}` would capture a free `{new_name}` reference in this scope"
            ));
        }
    }
    None
}

/// Every scope in which a declaration of `symbol` takes part in
/// redeclaration and merging rules.
///
/// That is the symbol's binding scope plus, for each declaration site, every
/// scope from the site up to the binding scope. A `var` declared in a block
/// binds in the function scope but also conflicts with a lexical declaration
/// in any block it hoists through, so all of those are included. A
/// `let`/`const`/`class`, a parameter, or a function declared directly in a
/// function body contributes only its own scope. A catch parameter is bound in
/// the catch body, so its clause scope is added as well.
///
/// A function declared in a block of sloppy-mode code also gets an Annex B
/// binding in the enclosing function scope, so its path extends there.
fn declaration_scopes(
    scoping: &Scoping,
    nodes: &AstNodes,
    sites: &[BindingSite],
    symbol: SymbolId,
) -> BTreeSet<ScopeId> {
    let binding = scoping.symbol_scope_id(symbol);
    let top = annex_b_var_scope(scoping, symbol).unwrap_or(binding);
    let mut scopes = BTreeSet::from([binding, top]);
    for site in sites.iter().filter(|site| site.symbol == symbol) {
        let start = site_scope(nodes, site.node);
        let path: Vec<ScopeId> = scoping.scope_ancestors(start).collect();
        match path.iter().position(|&scope| scope == top) {
            Some(end) => scopes.extend(&path[..=end]),
            None => {
                scopes.insert(start);
            }
        }
    }
    scopes
}

/// The scope a declaration statement sits in. A function or class
/// declaration's name is visited inside its own scope, so use the scope of
/// the declaration node instead; an expression's name binds in its own scope.
fn site_scope(nodes: &AstNodes, node: NodeId) -> ScopeId {
    let parent = nodes.parent_id(node);
    let declaration = match nodes.kind(parent) {
        AstKind::Function(function) => function.is_declaration(),
        AstKind::Class(class) => class.is_declaration(),
        _ => false,
    };
    if declaration {
        nodes.get_node(parent).scope_id()
    } else {
        nodes.get_node(node).scope_id()
    }
}

/// The function (or program) scope that a sloppy-mode block-level function
/// declaration is also hoisted to under Annex B, if `symbol` is one.
fn annex_b_var_scope(scoping: &Scoping, symbol: SymbolId) -> Option<ScopeId> {
    let binding = scoping.symbol_scope_id(symbol);
    let flags = scoping.scope_flags(binding);
    if !scoping.symbol_flags(symbol).is_function() || flags.is_var() || flags.is_strict_mode() {
        return None;
    }
    scoping
        .scope_ancestors(binding)
        .find(|&scope| scoping.scope_flags(scope).is_var())
}

/// For a sloppy-mode block-level function, a reference to its name outside
/// the block that Oxc leaves unresolved but Annex B binds to the function at
/// run time. Renaming would leave that reference behind.
fn annex_b_outer_reference(
    scoping: &Scoping,
    nodes: &AstNodes,
    symbol: SymbolId,
    original: &str,
    unresolved: &[(Span, String)],
) -> Option<Span> {
    let region = scope_span(scoping, nodes, annex_b_var_scope(scoping, symbol)?);
    unresolved
        .iter()
        .find(|(span, name)| name == original && contains(region, *span))
        .map(|(span, _)| *span)
}

/// The source region covering every declaration scope; the scopes form one
/// ancestor chain per site, so the widest span covers all of them.
fn declaration_region(scoping: &Scoping, nodes: &AstNodes, scopes: &BTreeSet<ScopeId>) -> Span {
    scopes
        .iter()
        .map(|&scope| scope_span(scoping, nodes, scope))
        .reduce(|widest, span| Span::new(widest.start.min(span.start), widest.end.max(span.end)))
        .unwrap_or_default()
}

fn collect_unresolved(program: &Program, scoping: &Scoping) -> Vec<(Span, String)> {
    struct Collect<'s> {
        scoping: &'s Scoping,
        out: Vec<(Span, String)>,
    }
    impl<'s, 'a> Visit<'a> for Collect<'s> {
        fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
            if analysis::identifier_symbol(identifier, self.scoping).is_none() {
                self.out
                    .push((identifier.span, identifier.name.to_string()));
            }
        }
    }
    let mut collect = Collect {
        scoping,
        out: Vec::new(),
    };
    collect.visit_program(program);
    collect.out
}

fn scope_span(scoping: &Scoping, nodes: &AstNodes, scope: ScopeId) -> Span {
    nodes.kind(scoping.get_node_id(scope)).span()
}

fn span_range(span: Span) -> SpanRange {
    (span.start, span.end)
}

fn same_span(a: Span, b: Span) -> bool {
    a.start == b.start && a.end == b.end
}

fn contains(outer: Span, inner: Span) -> bool {
    inner.start >= outer.start && inner.end <= outer.end
}

fn diagnostic(
    code: DiagnosticCode,
    message: String,
    span: Option<SpanRange>,
) -> ProgramTransformDiagnostic {
    ProgramTransformDiagnostic {
        code,
        message,
        span,
    }
}

/// Reserved words that cannot be used as a binding name, plus `eval` and
/// `arguments`, whose shadowing changes semantics.
fn is_reserved_word(name: &str) -> bool {
    matches!(
        name,
        "break"
            | "case"
            | "catch"
            | "class"
            | "const"
            | "continue"
            | "debugger"
            | "default"
            | "delete"
            | "do"
            | "else"
            | "enum"
            | "export"
            | "extends"
            | "false"
            | "finally"
            | "for"
            | "function"
            | "if"
            | "import"
            | "in"
            | "instanceof"
            | "new"
            | "null"
            | "return"
            | "super"
            | "switch"
            | "this"
            | "throw"
            | "true"
            | "try"
            | "typeof"
            | "var"
            | "void"
            | "while"
            | "with"
            | "yield"
            | "let"
            | "static"
            | "await"
            | "implements"
            | "interface"
            | "package"
            | "private"
            | "protected"
            | "public"
            | "eval"
            | "arguments"
    )
}

pub(super) fn is_valid_name(name: &str) -> bool {
    analysis::is_identifier_name(name) && !is_reserved_word(name)
}
