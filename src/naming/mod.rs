//! Function naming: small static function evaluators over one scoped parse
//! (`name_threads`), the collector identities, the semantic naming plan, the
//! function index / role graph inventory, class proofs and the program
//! rewrite engine. Readable bindings are retained except for authoritative
//! proven grouped registry positions.
use crate::signals::extract::bindings::{call_arguments, property_key_name, static_member_name, FunctionSite, Model};
use crate::naming::collectors::{identify_collectors, CollectorReport};
use crate::signals::schema::{BindingRef, Confidence, ProgramPropertiesLimits, SourceIdentity, SourceSpan};
use crate::naming::program::{
    planned_binding_facts_for, planned_bindings_collide, transform_program, PlannedBindingFacts,
    ProgramBindingRename, ProgramTransformOptions,
};
use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstKind};
use oxc_ast_visit::Visit;
use oxc_semantic::{NodeId, Scoping, SymbolId};
use oxc_syntax::operator::{AssignmentOperator, LogicalOperator};
use oxc_syntax::scope::ScopeId;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;
use crate::signals::extract::bindings::expression_path;
pub mod classes;
mod collector_payload;
pub mod collectors;
pub mod function_index;
pub mod inputs_context;
pub(crate) mod functions;
pub mod labels;
mod native_classes;
mod operations;
pub mod output;
pub mod program;
mod resolve;
pub mod role_graph;
pub mod semantic_names;
mod shared;
mod stages;
mod support;
use functions::{browser, encoding, events, graphics, network, observables, runtime, scheduling};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ThreadName {
    pub function_id: String,
    pub binding: BindingRef,
    pub thread_id: Option<u64>,
    pub pc: Option<u64>,
    /// Producer collision suffix, retained separately from the complete binding identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disambiguator: Option<u64>,
    pub proposed_name: Option<String>,
    /// Canonical semantic role shared by every distinct positive identification
    /// of this body, kept separate from the concrete unique binding name
    /// (`proposed_name`) chosen for this seed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role_name: Option<String>,
    pub evaluator: Option<String>,
    pub confidence: Confidence,
    pub evidence: Vec<SourceSpan>,
    pub explanation: Vec<String>,
    pub applied: bool,
    pub reason: Option<String>,
    pub reference_edits: usize,
    pub shorthand_edits: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ThreadNamingReport {
    /// `2` since the capability refactor: evaluator ids describe the rule
    /// capability (`events::…`, `signals::…`) instead of a vendor family, and
    /// multi-capability sources report one id per role. Emitted JavaScript,
    /// role names, confidence, evidence and edit ledgers are unchanged.
    pub schema_version: u32,
    pub source: SourceIdentity,
    pub transformed_source: SourceIdentity,
    pub function_count: usize,
    pub existing_named_function_count: usize,
    pub generated_thread_count: usize,
    pub applied_count: usize,
    pub outcomes: Vec<ThreadName>,
    /// Native class bindings are never included in the function counters above.
    #[serde(default)]
    pub generated_class_count: usize,
    #[serde(default)]
    pub applied_class_count: usize,
    #[serde(default)]
    pub unresolved_class_count: usize,
    #[serde(default)]
    pub class_outcomes: Vec<ThreadClassName>,
    pub removals: Vec<StubRemoval>,
    pub reference_matches: Vec<crate::naming::labels::LabelMatch>,
    pub function_roles: Vec<FunctionRole>,
    pub aggregate_collector_claims: Vec<AggregateCollectorClaim>,
    pub edits: Vec<crate::naming::program::ProgramSourceEdit>,
    pub collectors: CollectorReport,
    pub timings_ms: BTreeMap<String, f64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ThreadClassName {
    pub class_id: String,
    pub class_span: SourceSpan,
    pub binding: BindingRef,
    pub proposed_name: Option<String>,
    pub evaluator: Option<String>,
    pub confidence: Confidence,
    pub evidence: Vec<SourceSpan>,
    pub explanation: Vec<String>,
    pub applied: bool,
    pub reason: Option<String>,
    pub reference_edits: usize,
    pub shorthand_edits: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StubRemoval {
    pub name: String,
    pub span: SourceSpan,
    pub applied: bool,
    pub reason: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FunctionRole {
    pub function_span: SourceSpan,
    pub current_name: Option<String>,
    pub candidates: Vec<String>,
    pub evaluators: Vec<String>,
    pub status: String,
    pub confidence: Vec<Confidence>,
    pub evidence: Vec<SourceSpan>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AggregateCollectorClaim {
    pub function_span: SourceSpan,
    pub purpose_names: Vec<String>,
    pub reason: String,
}
pub struct NamedThreads {
    pub source: String,
    pub report: ThreadNamingReport,
}

#[derive(Default)]
struct Facts<'a> {
    calls: Vec<&'a CallExpression<'a>>,
    assignments: Vec<&'a AssignmentExpression<'a>>,
    returns: Vec<&'a Expression<'a>>,
    members: BTreeSet<String>,
    writes: BTreeSet<String>,
    globals: BTreeSet<String>,
    regexes: Vec<String>,
    statements: usize,
    loops: usize,
    deletes: usize,
    throws: usize,
    news: usize,
    class_super_slots: Vec<u32>,
    evidence: Vec<SourceSpan>,
}
#[derive(Clone)]
struct Proposal {
    name: String,
    evaluator: String,
    confidence: Confidence,
    evidence: Vec<SourceSpan>,
    explanation: Vec<String>,
}
fn proposal(name: &str, evaluator: &str, detail: &str, facts: &Facts<'_>) -> Proposal {
    Proposal {
        name: name.into(),
        evaluator: evaluator.into(),
        confidence: match evaluator {
            "hardcoded_serialization_error"
            | "constant_return"
            | "formal_return"
            | "prototype_assignment"
            | "resolved_api_alias"
            | "method_forwarder" => Confidence::High,
            _ => Confidence::Medium,
        },
        evidence: facts.evidence.clone(),
        explanation: vec![detail.into()],
    }
}

/// A concrete naming decision for one eligible generated binding.
enum DuplicateAllocation {
    /// The validator should be asked to apply this concrete name.
    Concrete { role: String, name: String },
    /// The group could not be allocated safely (missing declaration facts or
    /// an exhausted ordinal search), so the pre-existing refusal is kept.
    Blocked { role: String },
}

struct DuplicateCandidate {
    site_start: u32,
    declaration_start: u32,
    original_name: String,
    role_name: String,
    scope: ScopeId,
    disambiguator: Option<u64>,
    facts: Option<PlannedBindingFacts>,
}

/// Deterministically give every positively identified duplicate function a
/// distinct, scope-safe concrete name while keeping the meaningful semantic
/// base for bindings the older policy already applied.
///
/// The older policy applied a binding unless (a) another plan in the batch gave
/// a different binding the same name where the exact validator says they merge
/// or capture, or (b) a suffixed clone carried the same semantic name as an
/// unsuffixed role in an ancestor/descendant scope. Those applied bindings are
/// survivors and keep the unsuffixed base. Each refused member is then offered
/// `role`, `role2`, `role3`, ... in stable declaration order, skipping names
/// that already exist in the program (so human, builtin and unresolved
/// spellings are never displaced) and names another generated plan already
/// uses.
///
/// Solo claims are returned unchanged, so solo label behaviour and every
/// previously applied name are preserved. Sibling scopes that the validator
/// proves independent still share the base. The exact renamer re-checks every
/// concrete name; this pass only removes the *simultaneous* duplicate refusal.
fn allocate_duplicate_role_names(
    program: &Program,
    model: &Model<'_, '_>,
    proposals: &BTreeMap<u32, Vec<Proposal>>,
    rename_scope: &impl Fn(SymbolId) -> ScopeId,
) -> BTreeMap<u32, DuplicateAllocation> {
    let mut candidates: Vec<DuplicateCandidate> = Vec::new();
    for site in model.all_sites() {
        let Some(symbol) = site.binding_symbol else {
            continue;
        };
        let binding = model.binding_ref(symbol);
        if !binding.name.starts_with("$THREAD_") {
            continue;
        }
        let Some((_, _, disambiguator)) = thread_id(&binding.name) else {
            continue;
        };
        if model.binding_writes(symbol) != 0 {
            continue;
        }
        let Some(list) = proposals.get(&site.span.start) else {
            continue;
        };
        let mut names = list.iter().map(|p| p.name.as_str());
        let Some(role) = names.next() else {
            continue;
        };
        if names.any(|name| name != role) {
            continue;
        }
        candidates.push(DuplicateCandidate {
            site_start: site.span.start,
            declaration_start: binding.identifier.start,
            original_name: binding.name.clone(),
            role_name: role.to_string(),
            scope: rename_scope(symbol),
            disambiguator,
            facts: None,
        });
    }

    let mut groups: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (index, candidate) in candidates.iter().enumerate() {
        groups
            .entry(candidate.role_name.clone())
            .or_default()
            .push(index);
    }

    let mut allocations = BTreeMap::new();
    // A solo claim is exactly the existing label: never suffix it.
    for (role, indices) in &groups {
        if indices.len() == 1 {
            if let Some(candidate) = candidates.get(indices[0]) {
                allocations.insert(
                    candidate.site_start,
                    DuplicateAllocation::Concrete {
                        role: role.clone(),
                        name: role.clone(),
                    },
                );
            }
        }
    }

    // Reserve every spelling that already exists in the program, including
    // readable/human bindings and unresolved free references, so a generated
    // duplicate never reuses or shadows one of them. A base spelling owned by
    // such a name is skipped and the group starts at the next free ordinal.
    let mut reserved = unresolved_identifier_names(program, model.scoping);
    for symbol in model.scoping.symbol_ids() {
        reserved.insert(model.scoping.symbol_name(symbol).to_owned());
    }
    let mut vocabulary: BTreeSet<String> = BTreeSet::new();
    for list in proposals.values() {
        for entry in list {
            vocabulary.insert(entry.name.clone());
        }
    }

    for (role, indices) in &groups {
        if indices.len() < 2 {
            continue;
        }

        let starts: Vec<u32> = indices
            .iter()
            .map(|&index| candidates[index].declaration_start)
            .collect();
        let collected =
            planned_binding_facts_for(program, model.scoping, model.nodes(), &starts, role);
        let mut complete = true;
        for (&index, facts) in indices.iter().zip(collected) {
            match facts {
                Some(facts) => candidates[index].facts = Some(facts),
                None => complete = false,
            }
        }
        if !complete {
            for &index in indices {
                allocations.insert(
                    candidates[index].site_start,
                    DuplicateAllocation::Blocked { role: role.clone() },
                );
            }
            continue;
        }

        // Reconstruct the old applied/refused split exactly. A suffixed clone
        // that reserved an unsuffixed role in a related scope never became a
        // pending plan, and a per-plan name conflict or Annex B refusal kept a
        // binding out of the batch too. Batch collisions are therefore only
        // computed over bindings the old planner actually kept pending.
        let mut suffix_blocked = vec![false; indices.len()];
        for (position, &index) in indices.iter().enumerate() {
            for &other_index in indices {
                if other_index == index {
                    continue;
                }
                if candidates[index].disambiguator.is_some()
                    && candidates[other_index].disambiguator.is_none()
                    && scopes_related(
                        model.scoping,
                        candidates[index].scope,
                        candidates[other_index].scope,
                    )
                {
                    suffix_blocked[position] = true;
                }
            }
        }
        let pending_eligible: Vec<bool> = indices
            .iter()
            .map(|&index| {
                candidates[index]
                    .facts
                    .as_ref()
                    .is_some_and(|facts| !facts.name_conflicts && !facts.annex_b_excluded)
            })
            .collect();
        let mut refused = suffix_blocked.clone();
        for position in 0..indices.len() {
            if suffix_blocked[position] || !pending_eligible[position] {
                refused[position] = true;
                continue;
            }
            let Some(facts) = &candidates[indices[position]].facts else {
                refused[position] = true;
                continue;
            };
            for other_position in 0..indices.len() {
                if other_position == position
                    || suffix_blocked[other_position]
                    || !pending_eligible[other_position]
                {
                    continue;
                }
                let Some(other_facts) = &candidates[indices[other_position]].facts else {
                    refused[position] = true;
                    continue;
                };
                if planned_bindings_collide(model.scoping, model.nodes(), facts, other_facts) {
                    refused[position] = true;
                }
            }
        }

        // Members the old policy applied keep their meaningful unsuffixed base.
        let mut used: BTreeSet<String> = BTreeSet::new();
        for (position, &index) in indices.iter().enumerate() {
            if refused[position] {
                continue;
            }
            used.insert(role.clone());
            allocations.insert(
                candidates[index].site_start,
                DuplicateAllocation::Concrete {
                    role: role.clone(),
                    name: role.clone(),
                },
            );
        }

        // Refused duplicates get the first free ordinal in stable source order.
        let mut redeclared: Vec<usize> = indices
            .iter()
            .enumerate()
            .filter(|(position, _)| refused[*position])
            .map(|(_, &index)| index)
            .collect();
        redeclared.sort_by(|&first, &second| {
            candidates[first]
                .declaration_start
                .cmp(&candidates[second].declaration_start)
                .then_with(|| {
                    candidates[first]
                        .original_name
                        .cmp(&candidates[second].original_name)
                })
        });
        for index in redeclared {
            // An Annex B refusal is independent of the requested spelling, so
            // an ordinal cannot rescue it. Leave its fallback base plan to the
            // exact validator and do not consume a name other members can use.
            if candidates[index]
                .facts
                .as_ref()
                .is_some_and(|facts| facts.annex_b_excluded)
            {
                continue;
            }
            let mut chosen = None;
            for ordinal in 0..=(indices.len() + 2) {
                let candidate_name = if ordinal == 0 {
                    role.clone()
                } else {
                    format!("{role}{}", ordinal + 1)
                };
                if used.contains(&candidate_name) {
                    continue;
                }
                if reserved.contains(&candidate_name) {
                    continue;
                }
                if candidate_name != *role && vocabulary.contains(&candidate_name) {
                    continue;
                }
                chosen = Some(candidate_name);
                break;
            }
            match chosen {
                Some(name) => {
                    used.insert(name.clone());
                    allocations.insert(
                        candidates[index].site_start,
                        DuplicateAllocation::Concrete {
                            role: role.clone(),
                            name,
                        },
                    );
                }
                None => {
                    allocations.insert(
                        candidates[index].site_start,
                        DuplicateAllocation::Blocked { role: role.clone() },
                    );
                }
            }
        }
    }
    allocations
}

/// Is one scope the same as, or an ancestor/descendant of, the other?
fn scopes_related(scoping: &Scoping, first: ScopeId, second: ScopeId) -> bool {
    scoping
        .scope_ancestors(first)
        .any(|ancestor| ancestor == second)
        || scoping
            .scope_ancestors(second)
            .any(|ancestor| ancestor == first)
}

/// Every identifier reference Oxc leaves unresolved, i.e. a free global.
fn unresolved_identifier_names(program: &Program, scoping: &Scoping) -> BTreeSet<String> {
    struct Collect<'s> {
        scoping: &'s Scoping,
        names: BTreeSet<String>,
    }
    impl<'s, 'a> Visit<'a> for Collect<'s> {
        fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
            if crate::analysis::identifier_symbol(identifier, self.scoping).is_none() {
                self.names.insert(identifier.name.to_string());
            }
        }
    }
    let mut collect = Collect {
        scoping,
        names: BTreeSet::new(),
    };
    collect.visit_program(program);
    collect.names
}

/// Name generated functions and apply authoritative grouped registry positions.
/// Other readable names stay intact; separately reported, proven unreferenced
/// empty scoped declarations may be removed when editing is enabled.
pub fn name_threads(source: &str, rename: bool) -> Result<NamedThreads, String> {
    name_threads_with_labels(source, rename, None)
}
pub fn name_threads_with_labels(
    source: &str,
    rename: bool,
    reference: Option<&crate::naming::labels::LabelReference>,
) -> Result<NamedThreads, String> {
    let total = Instant::now();
    let mut timings = BTreeMap::new();
    let allocator = Allocator::default();
    let program = crate::analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = crate::analysis::analyze(&program);
    let model = Model::new(
        source,
        &program,
        &scoping,
        &nodes,
        ProgramPropertiesLimits::default(),
    )
    .map_err(|e| e.to_string())?;
    timings.insert(
        "parse_scope_index".into(),
        total.elapsed().as_secs_f64() * 1000.0,
    );
    let start = Instant::now();
    let mut facts: BTreeMap<u32, Facts<'_>> = BTreeMap::new();
    for node in nodes.iter() {
        let Some(owner) = model.owning_site(node.id()) else {
            continue;
        };
        let own = facts.entry(owner.span.start).or_default();
        match node.kind() {
            AstKind::Class(c) => {
                if let Some(super_) = &c.super_class {
                    if let Some(slot) = model.site_parameter_slot(super_, owner) {
                        own.class_super_slots.push(slot);
                    }
                }
            }
            AstKind::CallExpression(c) => {
                own.calls.push(c);
                own.evidence.push(c.span.into());
            }
            AstKind::ReturnStatement(r) => {
                if let Some(e) = &r.argument {
                    own.returns.push(e);
                    own.evidence.push(r.span.into());
                }
            }
            AstKind::StaticMemberExpression(m) => {
                own.members.insert(m.property.name.to_string());
            }
            AstKind::AssignmentExpression(a) => {
                own.assignments.push(a);
                own.evidence.push(a.span.into());
                if let AssignmentTarget::StaticMemberExpression(m) = &a.left {
                    own.writes.insert(m.property.name.to_string());
                }
            }
            AstKind::IdentifierReference(i)
                if crate::analysis::identifier_symbol(i, &scoping).is_none() =>
            {
                own.globals.insert(i.name.to_string());
            }
            AstKind::RegExpLiteral(r) => own.regexes.push(r.regex.pattern.text.to_string()),
            AstKind::ForStatement(_)
            | AstKind::ForInStatement(_)
            | AstKind::ForOfStatement(_)
            | AstKind::WhileStatement(_)
            | AstKind::DoWhileStatement(_) => own.loops += 1,
            AstKind::UnaryExpression(u) if u.operator.as_str() == "delete" => own.deletes += 1,
            AstKind::ThrowStatement(_) => own.throws += 1,
            AstKind::NewExpression(_) => own.news += 1,
            _ => (),
        }
    }
    for site in model.all_sites() {
        if let AstKind::Function(f) = nodes.get_node(site.node_id).kind() {
            facts.entry(site.span.start).or_default().statements =
                f.body.as_ref().map_or(0, |b| b.statements.len());
        }
    }
    let mut proposals: BTreeMap<u32, Vec<Proposal>> = BTreeMap::new();
    // Reuse the established, independent serializer evaluators. Calling this
    // package directly does not construct the unified naming plan or role graph.
    let ctx = crate::signals::analysis::roles::FinderCtx {
        source,
        program: &program,
        scoping: &scoping,
        nodes: &nodes,
    };
    let finder_claims = crate::signals::analysis::roles::function_claims_timed(&ctx, &mut timings);
    let class_claims = shared::native_class_claims(&model, &finder_claims);
    for claim in finder_claims {
        if let crate::signals::analysis::roles::FinderClaim::Function {
            role,
            function_span,
            confidence,
            evidence,
            ..
        } = claim
        {
            if let Some(vocabulary) = crate::signals::analysis::roles::finder_role(role) {
                proposals
                    .entry(function_span.start)
                    .or_default()
                    .push(Proposal {
                        name: vocabulary.name.into(),
                        evaluator: format!("function_finder::{role}"),
                        confidence,
                        evidence: vec![function_span.into()],
                        explanation: evidence,
                    });
            }
        }
    }
    let purpose_start = Instant::now();
    for outcome in crate::signals::analysis::roles::purposes::find_purposes(source, &program, &scoping, &nodes) {
        if let crate::signals::analysis::roles::purposes::PurposeOutcome::Found(finding) = outcome {
            proposals
                .entry(finding.function_span.start)
                .or_default()
                .push(Proposal {
                    name: finding.action_name.into(),
                    evaluator: format!("purpose_finder::{}", finding.role),
                    confidence: finding.confidence,
                    evidence: vec![finding.function_span.into()],
                    explanation: finding.evidence,
                });
        }
    }
    timings.insert(
        "purpose_finders".into(),
        purpose_start.elapsed().as_secs_f64() * 1000.0,
    );
    for site in model.all_sites() {
        if let Some(f) = facts.get(&site.span.start) {
            if let Some(p) = evaluate(&model, *site, f, &facts) {
                // The package's more specific role wins over the local fallback.
                proposals.entry(site.span.start).or_insert_with(|| vec![p]);
            }
        }
    }
    let shared_start = Instant::now();
    let shared: Vec<_> = model
        .all_sites()
        .iter()
        .filter(|site| !proposals.contains_key(&site.span.start))
        .filter_map(|site| {
            let f = facts.get(&site.span.start)?;
            let claims = if let Some(p) = shared::evaluate_shared(&model, *site, f, &proposals) {
                vec![p]
            } else {
                stages::evaluate_additions(&model, *site, f, &proposals)
            };
            (!claims.is_empty()).then_some((site.span.start, claims))
        })
        .collect();
    for (span, claims) in shared {
        proposals.insert(span, claims);
    }
    timings.insert(
        "shared_scoped_rules".into(),
        shared_start.elapsed().as_secs_f64() * 1000.0,
    );
    // Late registration: preserve every earlier proposal.
    for site in model.all_sites() {
        if !proposals.contains_key(&site.span.start) {
            if let Some(f) = facts.get(&site.span.start) {
                let claims = scheduling::callback_variants::evaluate(&model, *site, f);
                if !claims.is_empty() {
                    proposals.insert(site.span.start, claims);
                }
            }
        }
    }
    // Pass-2 registration: preserve every earlier applied role.
    for site in model.all_sites() {
        if !proposals.contains_key(&site.span.start) {
            if let Some(f) = facts.get(&site.span.start) {
                let claims = events::error_listener::evaluate(&model, *site, f);
                if !claims.is_empty() {
                    proposals.insert(site.span.start, claims);
                }
            }
        }
    }
    // WORKER-ENVIRONMENT: post-shared pass over only sites left unclaimed by
    // package finders and all shared scoped rules. This preserves the current
    // inheritance precedence while allowing the bundle container/emitter to
    // reuse helper roles proved by the completed shared pass.
    let worker_environment_start = Instant::now();
    let worker_environment: Vec<_> = model
        .all_sites()
        .iter()
        .filter(|site| !proposals.contains_key(&site.span.start))
        .filter_map(|site| {
            let f = facts.get(&site.span.start)?;
            let claims = stages::evaluate_worker_environment(&model, *site, f, &proposals);
            (!claims.is_empty()).then_some((site.span.start, claims))
        })
        .collect();
    for (span, claims) in worker_environment {
        proposals.insert(span, claims);
    }
    timings.insert(
        "worker_environment_bundle".into(),
        worker_environment_start.elapsed().as_secs_f64() * 1000.0,
    );
    // Second registration: late fallback preserves prior claims.
    for site in model.all_sites() {
        if !proposals.contains_key(&site.span.start) {
            if let Some(f) = facts.get(&site.span.start) {
                let claims = browser::proxy_probe::evaluate(&model, *site, f);
                if !claims.is_empty() {
                    proposals.insert(site.span.start, claims);
                }
            }
        }
    }
    // Next registration 2: preserve all earlier claims.
    for site in model.all_sites() {
        if !proposals.contains_key(&site.span.start) {
            if let Some(f) = facts.get(&site.span.start) {
                let claims = encoding::ascii_label_predicate::evaluate(&model, *site, f);
                if !claims.is_empty() {
                    proposals.insert(site.span.start, claims);
                }
            }
        }
    }
    // A function factory's exact returned child inherits a role only when that
    // child's independent evaluator proved it and the returned alias has a
    // supported runtime origin. No arbitrary holder label is used.
    for site in model.all_sites() {
        if proposals.contains_key(&site.span.start) {
            continue;
        }
        let Some(f) = facts.get(&site.span.start) else {
            continue;
        };
        if f.returns.len() == 1 && f.calls.is_empty() && f.throws == 0 && f.loops == 0 {
            if let Some(child) = shared::stable_function(&model, f.returns[0], 0) {
                if child.span != site.span {
                    if let Some(p) = proposals
                        .get(&child.span.start)
                        .and_then(|p| (p.len() == 1).then(|| p[0].clone()))
                    {
                        let name = format!("create{}", upper_first(&p.name));
                        proposals.insert(
                            site.span.start,
                            vec![proposal(
                                &name,
                                "returned_function_factory",
                                "returns the independently proven nested function",
                                f,
                            )],
                        );
                    }
                }
            }
        }
    }
    timings.insert(
        "static_evaluators".into(),
        start.elapsed().as_secs_f64() * 1000.0,
    );
    let start = Instant::now();
    let mut collectors = identify_collectors(source, false)
        .map_err(|e| e.to_string())?
        .report;
    let mut aggregate_collector_claims = Vec::new();
    let grouped_spans: BTreeSet<_> = crate::naming::collectors::proven_group_positions(
        &collectors.wiring,
        &collectors.registered_groups,
    )
    .map(|(function, _)| function.span)
    .collect();
    let mut positional_spans = BTreeSet::new();
    // Scoped row evidence remains available to name original collector bindings.
    for candidate in &collectors.renames {
        if candidate.binding.is_none()
            && candidate.reason.as_deref() == Some("report only; rename disabled")
        {
            if let Some(function) = collectors
                .registered_groups
                .iter()
                .filter_map(|g| g.function.as_ref())
                .find(|f| format!("fn@{}..{}", f.span.start, f.span.end) == candidate.function_id)
            {
                if let Some(old) = proposals.get(&function.span.start) {
                    aggregate_collector_claims.push(AggregateCollectorClaim {
                        function_span: function.span,
                        purpose_names: old.iter().map(|p| p.name.clone()).collect(),
                        reason:
                            "anonymous grouped producer purpose observations retained as metadata"
                                .into(),
                    });
                }
                proposals.insert(function.span.start, vec![Proposal {
                    name: candidate.proposed_name.clone().unwrap(), evaluator: "collector_registry".into(),
                    confidence: Confidence::High, evidence: vec![function.span],
                    explanation: vec!["six-argument grouped registry position; private anonymous identity holder".into()],
                }]);
            }
        }
        if let (Some(binding), Some(name)) = (&candidate.binding, &candidate.proposed_name) {
            if candidate.reason.as_deref() == Some("report only; rename disabled") {
                if let Some(function) = model.all_sites().iter().find(|s| {
                    format!("fn@{}..{}", s.span.start, s.span.end) == candidate.function_id
                        && s.binding_symbol
                            .is_some_and(|b| b.index() as u32 == binding.symbol)
                }) {
                    let route_proposal = Proposal {
                        name: name.clone(),
                        evaluator: "collector_registry".into(),
                        confidence: Confidence::High,
                        evidence: vec![function.span.into()],
                        explanation: vec![
                            "resolved grouped registry position or scoped direct producer route"
                                .into(),
                        ],
                    };
                    if grouped_spans.contains(&function.span.into()) {
                        positional_spans.insert(function.span.start);
                        if let Some(old) = proposals.get(&function.span.start) {
                            aggregate_collector_claims.push(AggregateCollectorClaim{function_span:function.span.into(),purpose_names:old.iter().map(|p|p.name.clone()).collect(),reason:"multi-signal aggregate: keep purpose observations as metadata; rename only by proven registry wave position".into()});
                        }
                        if !binding.name.starts_with("$THREAD_") && binding.name != *name {
                            aggregate_collector_claims.push(AggregateCollectorClaim {
                                function_span: function.span.into(), purpose_names: vec![binding.name.clone()],
                                reason: "prior readable registry label retained as metadata; positional identity is authoritative".into(),
                            });
                        }
                        proposals.insert(function.span.start, vec![route_proposal]);
                    } else {
                        proposals
                            .entry(function.span.start)
                            .or_insert_with(|| vec![route_proposal]);
                    }
                }
            }
        }
    }
    // Refused registry bindings cannot fall back to a different semantic name.
    for span in &grouped_spans {
        if !positional_spans.contains(&span.start)
            && !collectors.renames.iter().any(|r| {
                r.function_id == format!("fn@{}..{}", span.start, span.end)
                    && r.binding.is_none()
                    && r.reason.as_deref() == Some("report only; rename disabled")
            })
        {
            if let Some(old) = proposals.remove(&span.start) {
                aggregate_collector_claims.push(AggregateCollectorClaim {
                    function_span: *span, purpose_names: old.iter().map(|p| p.name.clone()).collect(),
                    reason: "registry positional rename refused; alternate purpose naming retained only as metadata".into(),
                });
            }
        }
    }
    timings.insert(
        "collector_mapping".into(),
        start.elapsed().as_secs_f64() * 1000.0,
    );
    let start = Instant::now();
    let mut reference_matches = reference
        .map(|r| crate::naming::labels::match_labels(&model, r))
        .transpose()?
        .unwrap_or_default();
    for matched in &mut reference_matches {
        if matched.candidates.is_empty() {
            continue;
        }
        if grouped_spans.contains(&matched.function_span) {
            aggregate_collector_claims.push(AggregateCollectorClaim{function_span:matched.function_span,purpose_names:matched.candidates.clone(),reason:"inherited purpose is aggregate metadata, not a signal identity or transferable wave position".into()});
            matched.status = "aggregate_collector_metadata".into();
            continue;
        }
        let list = proposals.entry(matched.function_span.start).or_default();
        for name in &matched.candidates {
            list.push(Proposal {
                name: name.clone(),
                evaluator: "inherited_structural_label".into(),
                confidence: Confidence::Medium,
                evidence: vec![matched.function_span],
                explanation: vec![format!(
                    "exact literal/API/scoped structural signature {}; inherited reference {}",
                    matched.signature.as_deref().unwrap_or(""),
                    reference.unwrap().original_source.sha256
                )],
            });
        }
    }
    for list in proposals.values_mut() {
        shared::reconcile_agreed_aliases(list);
        shared::reconcile_synonyms(list);
    }
    timings.insert(
        "reference_matching".into(),
        start.elapsed().as_secs_f64() * 1000.0,
    );
    // LATE ENTRY FALLBACK: the listener-style bootstrap chain (and the still
    // unnamed MAIN factory of a publisher shell) is proposed only after every
    // existing source above has been collected: package finders, shared
    // evaluators, returned-factory inheritance, collector routes and reference
    // labels. One package-index walk serves the whole batch, and a span that
    // already has any proposal is left exactly as it was.
    let late_start = Instant::now();
    for claim in crate::signals::analysis::roles::late_entry_claims(&ctx) {
        let crate::signals::analysis::roles::FinderClaim::Function {
            role,
            function_span,
            confidence,
            evidence,
            ..
        } = claim
        else {
            continue;
        };
        if grouped_spans.contains(&function_span.into()) {
            continue;
        }
        let Some(vocabulary) = crate::signals::analysis::roles::finder_role(role) else {
            continue;
        };
        let list = proposals.entry(function_span.start).or_default();
        if list.is_empty() {
            list.push(Proposal {
                name: vocabulary.name.into(),
                evaluator: format!("late_entry_chain::{role}"),
                confidence,
                evidence: vec![function_span.into()],
                explanation: evidence,
            });
        }
    }
    timings.insert(
        "late_entry_chain".into(),
        late_start.elapsed().as_secs_f64() * 1000.0,
    );
    // Keystroke timing recorders: only sites left unclaimed by every earlier
    // source are considered, so every existing applied name is preserved.
    let interaction_timing_start = Instant::now();
    for site in model.all_sites() {
        if grouped_spans.contains(&site.span.into()) {
            continue;
        }
        if proposals.contains_key(&site.span.start) {
            continue;
        }
        let Some(f) = facts.get(&site.span.start) else {
            continue;
        };
        let claims = events::interaction_timing_recorders::evaluate(&model, *site, f);
        if !claims.is_empty() {
            proposals.insert(site.span.start, claims);
        }
    }
    timings.insert(
        "interaction_timing_recorders".into(),
        interaction_timing_start.elapsed().as_secs_f64() * 1000.0,
    );
    // Final fallback: preserve every prior proposal.
    for site in model.all_sites() {
        if grouped_spans.contains(&site.span.into()) {
            continue;
        }
        if !proposals.contains_key(&site.span.start) {
            if let Some(f) = facts.get(&site.span.start) {
                let claims = encoding::counterless_entropy_seed::evaluate(&model, *site, f);
                if !claims.is_empty() {
                    proposals.insert(site.span.start, claims);
                }
            }
        }
    }
    let mut function_roles: Vec<FunctionRole> = model
        .all_sites()
        .iter()
        .map(|site| {
            let binding = site.binding_symbol.map(|s| model.binding_ref(s));
            let current_name = binding.as_ref().map(|b| b.name.clone()).or_else(|| {
                match nodes.parent_kind(site.node_id) {
                    AstKind::MethodDefinition(m) => property_key_name(&m.key),
                    _ => None,
                }
            });
            let entries = proposals.get(&site.span.start);
            let candidates: Vec<_> = entries
                .into_iter()
                .flatten()
                .map(|p| p.name.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            let status = if candidates.len() > 1 {
                "ambiguous"
            } else if candidates.is_empty() {
                "unresolved"
            } else if binding.is_none() {
                "metadata_only"
            } else if current_name
                .as_deref()
                .is_some_and(|n| !n.starts_with("$THREAD_"))
            {
                "existing_readable"
            } else {
                "identified"
            };
            FunctionRole {
                function_span: site.span.into(),
                current_name,
                candidates,
                status: status.into(),
                evaluators: entries
                    .into_iter()
                    .flatten()
                    .map(|p| p.evaluator.clone())
                    .collect(),
                confidence: entries
                    .into_iter()
                    .flatten()
                    .map(|p| p.confidence)
                    .collect(),
                evidence: entries
                    .into_iter()
                    .flatten()
                    .flat_map(|p| p.evidence.clone())
                    .collect(),
            }
        })
        .collect();
    // Pre-allocate a concrete name for every positively identified duplicate.
    // The allocator mirrors the checked renamer's Annex B scope widening for
    // sloppy block functions; it is conservative reservation, not
    // callable-origin resolution or permission to rename.
    let rename_scope = |symbol: SymbolId| -> ScopeId {
        let binding = model.scoping.symbol_scope_id(symbol);
        let flags = model.scoping.scope_flags(binding);
        if model.scoping.symbol_flags(symbol).is_function()
            && !flags.is_var()
            && !flags.is_strict_mode()
        {
            model
                .scoping
                .scope_ancestors(binding)
                .find(|s| model.scoping.scope_flags(*s).is_var())
                .unwrap_or(binding)
        } else {
            binding
        }
    };
    let allocations = allocate_duplicate_role_names(&program, &model, &proposals, &rename_scope);
    let mut outcomes = Vec::new();
    let mut plans = Vec::new();
    let mut existing = 0;
    for site in model.all_sites() {
        let Some(binding) = site.binding_symbol.map(|s| model.binding_ref(s)) else {
            continue;
        };
        let positional = positional_spans.contains(&site.span.start);
        if !binding.name.starts_with("$THREAD_") && !positional {
            existing += 1;
            continue;
        }
        let identity = thread_id(&binding.name);
        let list = proposals.get(&site.span.start);
        let names: BTreeSet<_> = list
            .into_iter()
            .flatten()
            .map(|p| p.name.as_str())
            .collect();
        let f = facts.get(&site.span.start);
        let mut reason = if names.len() > 1 {
            Some("conflicting static evaluator names".into())
        } else if list.is_none() {
            Some(
                if f.is_some_and(|f| f.statements == 0) {
                    "empty stub deferred; body retained"
                } else {
                    "no supported source-backed evaluator; body retained"
                }
                .into(),
            )
        } else {
            None
        };
        if identity.is_none() && !positional {
            reason = Some("unsupported generated function name grammar; body retained".into());
        }
        if model.binding_writes(site.binding_symbol.unwrap()) != 0 {
            reason = Some("function binding is reassigned".into());
        }
        let p = list.and_then(|p| p.first());
        let mut concrete_name = p.map(|p| p.name.clone());
        let mut role_name = p.map(|p| p.name.clone());
        if reason.is_none() {
            if let Some(allocation) = allocations.get(&site.span.start).filter(|_| !positional) {
                match allocation {
                    DuplicateAllocation::Concrete { role, name } => {
                        role_name = Some(role.clone());
                        concrete_name = Some(name.clone());
                    }
                    DuplicateAllocation::Blocked { role } => {
                        role_name = Some(role.clone());
                        concrete_name = None;
                        reason = Some(
                            "duplicate role has no safe concrete name; binding retained".into(),
                        );
                    }
                }
            }
        }
        if rename && reason.is_none() {
            if let Some(name) = &concrete_name {
                plans.push(ProgramBindingRename::new(binding.identifier.start, name));
            }
        }
        outcomes.push(ThreadName {
            function_id: format!("fn@{}..{}", site.span.start, site.span.end),
            binding,
            thread_id: identity.map(|i| i.0),
            pc: identity.map(|i| i.1),
            disambiguator: identity.and_then(|i| i.2),
            proposed_name: concrete_name,
            role_name,
            evaluator: p.map(|p| p.evaluator.clone()),
            confidence: p.map_or(Confidence::Unresolved, |p| p.confidence),
            evidence: p.map_or_else(Vec::new, |p| p.evidence.clone()),
            explanation: p.map_or_else(Vec::new, |p| p.explanation.clone()),
            applied: false,
            reason: reason.or_else(|| (!rename).then(|| "report only; rename disabled".into())),
            reference_edits: 0,
            shorthand_edits: 0,
        });
    }
    // Class proof/application is additive. Reserve every function proposal so
    // a new class candidate can never invalidate an established function plan.
    let mut class_outcomes = native_classes::plan(&program, &model, &class_claims, &mut plans, rename);
    let start = Instant::now();
    let mut edits = Vec::new();
    let mut transformed = if plans.is_empty() {
        source.to_string()
    } else {
        let result = transform_program(source, &plans, &[], &ProgramTransformOptions::default())
            .map_err(|e| e.to_string())?;
        for outcome in &result.renames {
            if let Some(entry) = outcomes
                .iter_mut()
                .find(|e| e.binding.identifier.start == outcome.declaration_start)
            {
                entry.applied = outcome.applied;
                entry.reason = outcome.skip_reason.as_ref().map(|r| format!("{r:?}"));
                entry.reference_edits = outcome.reference_edits;
                entry.shorthand_edits = outcome.shorthand_edits;
            }
            if let Some(entry) = class_outcomes.iter_mut().find(|e| e.binding.identifier.start == outcome.declaration_start) {
                entry.applied = outcome.applied;
                entry.reason = outcome.skip_reason.as_ref().map(|r| format!("{r:?}"));
                entry.reference_edits = outcome.reference_edits;
                entry.shorthand_edits = outcome.shorthand_edits;
            }
        }
        edits = result.edits;
        result.transformed_source
    };
    let mut removals = Vec::new();
    if !crate::naming::program::has_dynamic_scope(&program) {
        for node in nodes.iter() {
            let AstKind::Function(f) = node.kind() else {
                continue;
            };
            if f.r#type != FunctionType::FunctionDeclaration {
                continue;
            }
            // Sloppy block declarations can have Annex B uses outside their
            // lexical block which the scope table leaves unresolved.
            if !matches!(nodes.parent_kind(node.id()), AstKind::FunctionBody(_)) {
                continue;
            }
            let (Some(id), Some(body)) = (&f.id, &f.body) else {
                continue;
            };
            let Some(symbol) = id.symbol_id.get() else {
                continue;
            };
            // Unsupported generated identities remain explicitly unresolved,
            // including empty bodies; grammar expansion is not new DCE.
            if id.name.starts_with("$THREAD_") && thread_id(&id.name).is_none() {
                continue;
            }

            if !body.statements.is_empty() || !body.directives.is_empty()
                || !scoping.get_resolved_reference_ids(symbol).is_empty()
                || scoping.symbol_scope_id(symbol)==scoping.root_scope_id()
                || nodes.iter().filter(|n|matches!(n.kind(),AstKind::BindingIdentifier(b) if b.symbol_id.get()==Some(symbol))).count()!=1 {continue;}
            removals.push(StubRemoval {
                name: id.name.to_string(),
                span: standalone_declaration_span(source, f.span.into()),
                applied: rename,
                reason:
                    "unreferenced empty scoped declaration; no dynamic scope or duplicate binding"
                        .into(),
            });
        }
    }
    if rename && !removals.is_empty() {
        for removal in &removals {
            edits.retain(|e| !(e.start >= removal.span.start && e.end <= removal.span.end));
            for outcome in &mut outcomes {
                if outcome.binding.identifier.start >= removal.span.start
                    && outcome.binding.identifier.end <= removal.span.end
                {
                    outcome.applied = false;
                    outcome.reason = Some("removed unused empty declaration; see removals".into());
                }
            }
            for role in &mut function_roles {
                if role.function_span.start >= removal.span.start
                    && role.function_span.end <= removal.span.end
                {
                    role.status = "removed_empty".into();
                }
            }
            edits.push(crate::naming::program::ProgramSourceEdit {
                start: removal.span.start,
                end: removal.span.end,
                replacement: String::new(),
            });
        }
        // A removed declaration has no references and cannot overlap a checked
        // binding rename. Reapply both edit classes in original coordinates.
        let applied = crate::analysis::apply_edits(
            source,
            edits
                .iter()
                .map(|e| crate::analysis::Edit {
                    start: e.start,
                    end: e.end,
                    replacement: e.replacement.clone(),
                })
                .collect(),
        );
        if !applied.rejected.is_empty() {
            return Err("overlapping function edits/removals".into());
        }
        crate::naming::program::validate_rewrite(&program, &applied.output)
            .map_err(|error| format!("post-removal validation: {error:?}"))?;
        transformed = applied.output;
    }
    if rename && !collectors.anonymous_naming_edits.is_empty() {
        edits.extend(collectors.anonymous_naming_edits.clone());
        transformed = crate::naming::collectors::checked_identity_edits(source, &program, &edits)?;
        for candidate in &mut collectors.renames {
            if candidate.binding.is_none()
                && candidate.reason.as_deref() == Some("report only; rename disabled")
            {
                candidate.applied = true;
                candidate.reason = None;
            }
        }
    }
    timings.insert(
        "checked_thread_binding_rename".into(),
        start.elapsed().as_secs_f64() * 1000.0,
    );
    timings.insert("total".into(), total.elapsed().as_secs_f64() * 1000.0);
    edits.sort_by_key(|edit| (edit.start, edit.end));
    Ok(NamedThreads {
        report: ThreadNamingReport {
            schema_version: 2,
            source: collectors.source.clone(),
            transformed_source: SourceIdentity {
                sha256: crate::source::sha256_hex(transformed.as_bytes()),
                bytes: transformed.len(),
            },
            function_count: model.all_sites().len(),
            existing_named_function_count: existing,
            generated_thread_count: outcomes.len(),
            applied_count: outcomes.iter().filter(|o| o.applied).count(),
            outcomes,
            generated_class_count: class_outcomes.len(),
            applied_class_count: class_outcomes.iter().filter(|o| o.applied).count(),
            unresolved_class_count: class_outcomes.iter().filter(|o| !o.applied).count(),
            class_outcomes,
            removals,
            reference_matches,
            function_roles,
            aggregate_collector_claims,
            edits,
            collectors,
            timings_ms: timings,
        },
        source: transformed,
    })
}
fn standalone_declaration_span(source: &str, span: SourceSpan) -> SourceSpan {
    let start = span.start as usize;
    let end = span.end as usize;
    let line_start = source[..start].rfind('\n').map_or(0, |p| p + 1);
    let line_end = source[end..]
        .find('\n')
        .map_or(source.len(), |p| end + p + 1);
    if source[line_start..start].trim().is_empty() && source[end..line_end].trim().is_empty() {
        SourceSpan::new(line_start as u32, line_end as u32)
    } else {
        span
    }
}
// NameMap::reserve (tracing/symbolic/emit.rs) appends _2, _3, ... on
// collisions. Keep the full binding/span as identity; never join by the stem.
pub(crate) fn thread_id(name: &str) -> Option<(u64, u64, Option<u64>)> {
    let mut parts = name.strip_prefix("$THREAD_")?.split('_');
    let decimal = |s: &str| -> Option<u64> {
        (!s.is_empty() && s.bytes().all(|c| c.is_ascii_digit()))
            .then(|| s.parse().ok())
            .flatten()
    };
    let tid = decimal(parts.next()?)?;
    let pc = decimal(parts.next()?)?;
    let disambiguator = match parts.next() {
        Some(s) => {
            let n = decimal(s)?;
            if n < 2 || n.to_string() != s {
                return None;
            }
            Some(n)
        }
        None => None,
    };
    if parts.next().is_some() {
        return None;
    }
    Some((tid, pc, disambiguator))
}
fn upper_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}
fn has(f: &Facts<'_>, members: &[&str]) -> bool {
    members.iter().all(|m| f.members.contains(*m))
}
fn called(f: &Facts<'_>, method: &str) -> bool {
    f.calls
        .iter()
        .any(|c| static_member_name(&c.callee) == Some(method))
}
fn global_call(model: &Model<'_, '_>, f: &Facts<'_>, path: &[&str]) -> bool {
    f.calls
        .iter()
        .any(|c| expression_path(model, &c.callee) == path)
}
fn local_call(model: &Model<'_, '_>, f: &Facts<'_>, name: &str) -> bool {
    f.calls.iter().any(|c|matches!(&c.callee,Expression::Identifier(i) if i.name==name && crate::analysis::identifier_symbol(i,model.scoping).and_then(|s|model.function_from_symbol(s)).is_some()))
}
/// The function-probe record returned by name: a body-local assembled as a
/// placeholder object literal plus field writes. The record must hold exactly
/// one `record = { a: <typeof input === "function"> }` write and then the
/// distinct, ordered `record.b/.c/.d/.e` writes. Every field is bound to the
/// same computation the guard's member evidence names: `a` is the typeof
/// formal test through this body's stable input copy, `b` is the guarded
/// native `test(toString.call(input))` result, `c` is the negated guarded
/// prototype-presence call on the same input, `d` is the try/delete same-input
/// success flag (false before the try and in both fallbacks), and `e` is the
/// guarded source-text call or its empty fallback. A captured, rebuilt,
/// reordered, overwritten, foreign-valued or donor record refuses.
fn placeholder_probe_record(
    model: &Model<'_, '_>,
    site: FunctionSite,
    f: &Facts<'_>,
    returned: &Expression<'_>,
) -> bool {
    let Expression::Identifier(identifier) = returned.without_parentheses() else {
        return false;
    };
    let Some(record) = crate::analysis::identifier_symbol(identifier, model.scoping) else {
        return false;
    };
    if !declared_within_body(model, record, site) {
        return false;
    }
    let mut placeholder = None;
    let mut typed = None;
    let mut fields: Vec<(u32, &str, &Expression<'_>)> = Vec::new();
    for assignment in &f.assignments {
        match &assignment.left {
            AssignmentTarget::AssignmentTargetIdentifier(target)
                if crate::analysis::identifier_symbol(target, model.scoping) == Some(record) =>
            {
                let Expression::ObjectExpression(object) = assignment.right.without_parentheses()
                else {
                    return false;
                };
                let Some(symbol) = typed_placeholder_symbol(object, model, site, f) else {
                    return false;
                };
                if placeholder.is_some() {
                    return false;
                }
                placeholder = Some(assignment.span.start);
                typed = Some(symbol);
            }
            AssignmentTarget::StaticMemberExpression(member) => {
                if crate::analysis::expression_symbol(
                    member.object.without_parentheses(),
                    model.scoping,
                ) != Some(record)
                {
                    continue;
                }
                fields.push((
                    assignment.span.start,
                    member.property.name.as_str(),
                    &assignment.right,
                ));
            }
            _ => (),
        }
    }
    let (Some(placeholder), Some(typed)) = (placeholder, typed) else {
        return false;
    };
    fields.sort_by_key(|(start, _, _)| *start);
    let [b, c, d, e] = fields.as_slice() else {
        return false;
    };
    b.1 == "b"
        && c.1 == "c"
        && d.1 == "d"
        && e.1 == "e"
        && b.0 > placeholder
        && c.0 > placeholder
        && d.0 > placeholder
        && e.0 > placeholder
        && placeholder_field_dataflow(
            model,
            f,
            site,
            record,
            typed,
            placeholder,
            b.2,
            c.2,
            d.2,
            e.2,
        )
}
/// A single-field placeholder keyed `a` whose value is a body-local initialized
/// by a `typeof <input> === "function"` test, where `<input>` is this body's
/// stable copy of formal slot 0.
fn typed_placeholder_symbol(
    object: &ObjectExpression<'_>,
    model: &Model<'_, '_>,
    site: FunctionSite,
    f: &Facts<'_>,
) -> Option<SymbolId> {
    let [ObjectPropertyKind::ObjectProperty(property)] = object.properties.as_slice() else {
        return None;
    };
    if property_key_name(&property.key).as_deref() != Some("a") {
        return None;
    }
    let Expression::Identifier(value) = property.value.without_parentheses() else {
        return None;
    };
    let symbol = crate::analysis::identifier_symbol(value, model.scoping)?;
    if !declared_within_body(model, symbol, site) {
        return None;
    }
    let operand = typeof_function_operand(model.initializer(symbol)?)?;
    let before = model.binding_ref(symbol).declaration.start;
    input_at(model, f, site, operand, before).then_some(symbol)
}
fn placeholder_field_dataflow(
    model: &Model<'_, '_>,
    f: &Facts<'_>,
    site: FunctionSite,
    record: SymbolId,
    typed: SymbolId,
    record_start: u32,
    b_value: &Expression<'_>,
    c_value: &Expression<'_>,
    d_value: &Expression<'_>,
    e_value: &Expression<'_>,
) -> bool {
    // The `a` value's typeof operand is this body's stable copy of formal 0.
    if typeof_alias(model, site, typed).is_none() {
        return false;
    }
    if typed != record
        && f.assignments
            .iter()
            .any(|&assignment| assignment_symbol(model, assignment) == Some(typed))
    {
        return false;
    }
    // b: one guarded native-source test call and its false fallback.
    let Expression::Identifier(b_identifier) = b_value.without_parentheses() else {
        return false;
    };
    let Some(b_symbol) = crate::analysis::identifier_symbol(b_identifier, model.scoping) else {
        return false;
    };
    if !declared_within_body(model, b_symbol, site) || model.initializer(b_symbol).is_some() {
        return false;
    }
    let mut b_call = None;
    let mut b_false = None;
    for &assignment in &f.assignments {
        if assignment_symbol(model, assignment) != Some(b_symbol) {
            continue;
        }
        if assignment.operator != AssignmentOperator::Assign {
            return false;
        }
        match assignment.right.without_parentheses() {
            Expression::CallExpression(call) => {
                let Some(helper) = native_text_call(model, f, site, call) else {
                    return false;
                };
                if b_call.is_some() {
                    return false;
                }
                b_call = Some((assignment, call.span.start, helper));
            }
            Expression::BooleanLiteral(literal) if !literal.value => {
                if b_false.is_some() {
                    return false;
                }
                b_false = Some(assignment);
            }
            _ => return false,
        }
    }
    let (Some((b_assignment, b_start, source_helper)), Some(b_false)) = (b_call, b_false) else {
        return false;
    };
    if !guarded_pair(model, typed, b_assignment, b_false) {
        return false;
    }
    // c: negated guarded prototype-presence result for the same input.
    let Expression::UnaryExpression(c_negation) = c_value.without_parentheses() else {
        return false;
    };
    if c_negation.operator.as_str() != "!" {
        return false;
    }
    let Expression::Identifier(c_identifier) = c_negation.argument.without_parentheses() else {
        return false;
    };
    let Some(c_symbol) = crate::analysis::identifier_symbol(c_identifier, model.scoping) else {
        return false;
    };
    if !declared_within_body(model, c_symbol, site) || model.initializer(c_symbol).is_some() {
        return false;
    }
    let mut c_call = None;
    let mut c_false = None;
    for &assignment in &f.assignments {
        if assignment_symbol(model, assignment) != Some(c_symbol) {
            continue;
        }
        if assignment.operator != AssignmentOperator::Assign {
            return false;
        }
        match assignment.right.without_parentheses() {
            Expression::CallExpression(call) => {
                if !prototype_presence_call(model, f, site, call) || c_call.is_some() {
                    return false;
                }
                c_call = Some((assignment, call.span.start));
            }
            Expression::BooleanLiteral(literal) if !literal.value => {
                if c_false.is_some() {
                    return false;
                }
                c_false = Some(assignment);
            }
            _ => return false,
        }
    }
    let (Some((c_assignment, c_start)), Some(c_false)) = (c_call, c_false) else {
        return false;
    };
    if !guarded_pair(model, typed, c_assignment, c_false) {
        return false;
    }
    // d: try/delete success flag, false before the try and in both fallbacks.
    let Expression::Identifier(d_identifier) = d_value.without_parentheses() else {
        return false;
    };
    let Some(d_symbol) = crate::analysis::identifier_symbol(d_identifier, model.scoping) else {
        return false;
    };
    if !declared_within_body(model, d_symbol, site) || model.initializer(d_symbol).is_some() {
        return false;
    }
    let mut d_true = None;
    let mut d_false = 0usize;
    for &assignment in &f.assignments {
        if assignment_symbol(model, assignment) != Some(d_symbol) {
            continue;
        }
        if assignment.operator != AssignmentOperator::Assign {
            return false;
        }
        match assignment.right.without_parentheses() {
            Expression::BooleanLiteral(literal) if literal.value => {
                if d_true.is_some() {
                    return false;
                }
                d_true = Some(assignment);
            }
            Expression::BooleanLiteral(_) => d_false += 1,
            _ => return false,
        }
    }
    let Some(d_true_assignment) = d_true else {
        return false;
    };
    if d_false != 2
        || !deleted_prototype_guard(model, f, site, typed, c_symbol, d_symbol, d_true_assignment)
    {
        return false;
    }
    // e: the guarded source-text call (or empty fallback) on the same input.
    let Expression::Identifier(e_identifier) = e_value.without_parentheses() else {
        return false;
    };
    let Some(e_symbol) = crate::analysis::identifier_symbol(e_identifier, model.scoping) else {
        return false;
    };
    if !declared_within_body(model, e_symbol, site) {
        return false;
    }
    let Some(e_guard) = source_text_guard(model, f, site, typed, e_symbol, source_helper) else {
        return false;
    };
    if e_guard.if_end > record_start
        || has_write_between(model, f, e_symbol, e_guard.if_end, record_start)
    {
        return false;
    }
    b_symbol != c_symbol
        && b_symbol != d_symbol
        && c_symbol != d_symbol
        && e_symbol != b_symbol
        && e_symbol != c_symbol
        && e_symbol != d_symbol
        && e_symbol != typed
        && b_symbol != typed
        && c_symbol != typed
        && d_symbol != typed
        && b_start < c_start
        && c_start < e_guard.call_start
        && e_guard.call_start < record_start
        && d_true_assignment.span.start < record_start
}

/// The body-local alias that the `a` typeof test reads and that only ever
/// starts from formal slot 0 of this function.
fn typeof_alias(model: &Model<'_, '_>, site: FunctionSite, typed: SymbolId) -> Option<SymbolId> {
    let operand = typeof_function_operand(model.initializer(typed)?)?;
    let Expression::Identifier(identifier) = operand.without_parentheses() else {
        return None;
    };
    let alias = crate::analysis::identifier_symbol(identifier, model.scoping)?;
    if !declared_within_body(model, alias, site) {
        return None;
    }
    let initializer = model.initializer(alias)?;
    (model.site_parameter_slot(initializer, site) == Some(0)).then_some(alias)
}

/// Whether `expression`, read at `before`, is formal slot 0 of `site` directly
/// or through a body-local whose latest definition before that point is that
/// same formal.
fn input_at(
    model: &Model<'_, '_>,
    f: &Facts<'_>,
    site: FunctionSite,
    expression: &Expression<'_>,
    before: u32,
) -> bool {
    if model.site_parameter_slot(expression, site) == Some(0) {
        return true;
    }
    let Some(symbol) =
        crate::analysis::expression_symbol(expression.without_parentheses(), model.scoping)
    else {
        return false;
    };
    if !declared_within_body(model, symbol, site) || model.formal_symbol_slot(symbol).is_some() {
        return false;
    }
    last_input_definition(model, f, site, symbol, before)
}

fn last_input_definition(
    model: &Model<'_, '_>,
    f: &Facts<'_>,
    site: FunctionSite,
    symbol: SymbolId,
    before: u32,
) -> bool {
    let mut last: Option<(u32, bool)> = None;
    if let Some(initializer) = model.initializer(symbol) {
        let start = model.binding_ref(symbol).declaration.start;
        if start < before {
            last = Some((
                start,
                model.site_parameter_slot(initializer, site) == Some(0),
            ));
        }
    }
    for &assignment in &f.assignments {
        if assignment.span.start >= before {
            continue;
        }
        if assignment_symbol(model, assignment) != Some(symbol) {
            continue;
        }
        if assignment.operator != AssignmentOperator::Assign {
            return false;
        }
        if last.is_none_or(|(start, _)| assignment.span.start >= start) {
            last = Some((
                assignment.span.start,
                model.site_parameter_slot(&assignment.right, site) == Some(0),
            ));
        }
    }
    last.is_some_and(|(_, input)| input)
}

/// A `B = <call>` write in the consequent of `if (<typed>)`, paired with the
/// one `B = false` write in that if's else block.
fn guarded_pair(
    model: &Model<'_, '_>,
    typed: SymbolId,
    call_assignment: &AssignmentExpression<'_>,
    false_assignment: &AssignmentExpression<'_>,
) -> bool {
    let Some(if_statement) = enclosing_if(model, call_assignment.node_id.get()) else {
        return false;
    };
    if !identifier_is(&if_statement.test, model, typed) {
        return false;
    }
    let Statement::BlockStatement(consequent) = &if_statement.consequent else {
        return false;
    };
    if !consequent.span.contains_inclusive(call_assignment.span) {
        return false;
    }
    let Some(Statement::BlockStatement(alternate)) = &if_statement.alternate else {
        return false;
    };
    let [Statement::ExpressionStatement(only)] = alternate.body.as_slice() else {
        return false;
    };
    let Expression::AssignmentExpression(inner) = only.expression.without_parentheses() else {
        return false;
    };
    inner.node_id.get() == false_assignment.node_id.get()
}

/// The guarded `pattern.test(fnToString.call(input))` shape of `b`. The
/// receiver must be a captured pattern, so a body-local donor call refuses.
fn native_text_call(
    model: &Model<'_, '_>,
    f: &Facts<'_>,
    site: FunctionSite,
    call: &CallExpression<'_>,
) -> Option<SymbolId> {
    if call.arguments.len() != 1 {
        return None;
    }
    let Expression::StaticMemberExpression(test) = &call.callee else {
        return None;
    };
    if test.property.name != "test" {
        return None;
    }
    let Expression::Identifier(pattern) = test.object.without_parentheses() else {
        return None;
    };
    let pattern = crate::analysis::identifier_symbol(pattern, model.scoping)?;
    if declared_within_body(model, pattern, site) || body_writes(model, f, pattern) {
        return None;
    }
    let args = call_arguments(call);
    let [argument] = args.as_slice() else {
        return None;
    };
    let Expression::CallExpression(inner) = argument.without_parentheses() else {
        return None;
    };
    if inner.arguments.len() != 1 {
        return None;
    }
    let Expression::StaticMemberExpression(source) = &inner.callee else {
        return None;
    };
    if source.property.name != "call" {
        return None;
    }
    let Expression::Identifier(helper) = source.object.without_parentheses() else {
        return None;
    };
    let helper = crate::analysis::identifier_symbol(helper, model.scoping)?;
    if body_writes(model, f, helper) {
        return None;
    }
    let inner_args = call_arguments(inner);
    let [input] = inner_args.as_slice() else {
        return None;
    };
    input_at(model, f, site, input, inner.span.start).then_some(helper)
}

/// The guarded prototype-presence call on the probe input, either
/// `hasOwn.call(input, "prototype")` or the captured `hasOwn(input, "prototype")`
/// form.
fn prototype_presence_call(
    model: &Model<'_, '_>,
    f: &Facts<'_>,
    site: FunctionSite,
    call: &CallExpression<'_>,
) -> bool {
    if call.arguments.len() != 2 {
        return false;
    }
    let args = call_arguments(call);
    let [input, key] = args.as_slice() else {
        return false;
    };
    if !matches!(
        key.without_parentheses(),
        Expression::StringLiteral(literal) if literal.value == "prototype"
    ) || !input_at(model, f, site, input, call.span.start)
    {
        return false;
    }
    match call.callee.without_parentheses() {
        Expression::StaticMemberExpression(member) => {
            if member.property.name != "call" {
                return false;
            }
            let Expression::Identifier(helper) = member.object.without_parentheses() else {
                return false;
            };
            crate::analysis::identifier_symbol(helper, model.scoping)
                .is_some_and(|helper| !body_writes(model, f, helper))
        }
        Expression::Identifier(helper) => crate::analysis::identifier_symbol(helper, model.scoping)
            .is_some_and(|helper| !body_writes(model, f, helper)),
        _ => false,
    }
}

/// `d` is assigned false before the try, set true only after
/// `delete <input>.prototype`, and assigned false in the else fallback. The
/// catch block is empty, so a thrown delete leaves the local false.
fn deleted_prototype_guard(
    model: &Model<'_, '_>,
    f: &Facts<'_>,
    site: FunctionSite,
    typed: SymbolId,
    c_symbol: SymbolId,
    d_symbol: SymbolId,
    true_assignment: &AssignmentExpression<'_>,
) -> bool {
    let Some(try_statement) = enclosing_try(model, true_assignment.node_id.get()) else {
        return false;
    };
    if try_statement.finalizer.is_some() {
        return false;
    }
    let Some(handler) = &try_statement.handler else {
        return false;
    };
    if !handler.body.body.is_empty() {
        return false;
    }
    let [Statement::ExpressionStatement(delete_statement), Statement::ExpressionStatement(success_statement)] =
        try_statement.block.body.as_slice()
    else {
        return false;
    };
    let Expression::UnaryExpression(delete) = delete_statement.expression.without_parentheses()
    else {
        return false;
    };
    if delete.operator.as_str() != "delete" {
        return false;
    }
    let Expression::StaticMemberExpression(deleted) = delete.argument.without_parentheses() else {
        return false;
    };
    if deleted.property.name != "prototype"
        || !input_at(model, f, site, &deleted.object, delete.span.start)
    {
        return false;
    }
    let Expression::AssignmentExpression(success) =
        success_statement.expression.without_parentheses()
    else {
        return false;
    };
    if success.node_id.get() != true_assignment.node_id.get() {
        return false;
    }
    let Some(if_statement) = enclosing_if(model, try_statement.node_id.get()) else {
        return false;
    };
    let Expression::LogicalExpression(guard) = if_statement.test.without_parentheses() else {
        return false;
    };
    if guard.operator != LogicalOperator::And || !identifier_is(&guard.left, model, typed) {
        return false;
    }
    let Expression::UnaryExpression(negated) = guard.right.without_parentheses() else {
        return false;
    };
    if negated.operator.as_str() != "!" || !identifier_is(&negated.argument, model, c_symbol) {
        return false;
    }
    let Statement::BlockStatement(consequent) = &if_statement.consequent else {
        return false;
    };
    let [Statement::ExpressionStatement(before_statement), Statement::TryStatement(inner_try)] =
        consequent.body.as_slice()
    else {
        return false;
    };
    if inner_try.node_id.get() != try_statement.node_id.get() {
        return false;
    }
    let Expression::AssignmentExpression(before) =
        before_statement.expression.without_parentheses()
    else {
        return false;
    };
    if assignment_symbol(model, before) != Some(d_symbol)
        || !matches!(
            before.right.without_parentheses(),
            Expression::BooleanLiteral(literal) if !literal.value
        )
    {
        return false;
    }
    let Some(Statement::BlockStatement(alternate)) = &if_statement.alternate else {
        return false;
    };
    let [Statement::ExpressionStatement(after_statement)] = alternate.body.as_slice() else {
        return false;
    };
    let Expression::AssignmentExpression(after) = after_statement.expression.without_parentheses()
    else {
        return false;
    };
    assignment_symbol(model, after) == Some(d_symbol)
        && matches!(
            after.right.without_parentheses(),
            Expression::BooleanLiteral(literal) if !literal.value
        )
}

struct SourceTextGuard {
    if_end: u32,
    call_start: u32,
}

/// The guarded `e = fnToString.call(input)` / `e = ""` assignment pair.
fn source_text_guard(
    model: &Model<'_, '_>,
    f: &Facts<'_>,
    site: FunctionSite,
    typed: SymbolId,
    symbol: SymbolId,
    helper: SymbolId,
) -> Option<SourceTextGuard> {
    let mut found = None;
    for &assignment in &f.assignments {
        if assignment_symbol(model, assignment) != Some(symbol) {
            continue;
        }
        if assignment.operator != AssignmentOperator::Assign {
            return None;
        }
        let Expression::CallExpression(call) = assignment.right.without_parentheses() else {
            continue;
        };
        if source_text_call(model, f, site, call) != Some(helper) {
            continue;
        }
        let Some(if_statement) = enclosing_if(model, assignment.node_id.get()) else {
            continue;
        };
        if !identifier_is(&if_statement.test, model, typed) {
            continue;
        }
        let Statement::BlockStatement(consequent) = &if_statement.consequent else {
            continue;
        };
        let [Statement::ExpressionStatement(only)] = consequent.body.as_slice() else {
            continue;
        };
        let Expression::AssignmentExpression(inner) = only.expression.without_parentheses() else {
            continue;
        };
        if inner.node_id.get() != assignment.node_id.get() {
            continue;
        }
        let Some(Statement::BlockStatement(alternate)) = &if_statement.alternate else {
            continue;
        };
        let [Statement::ExpressionStatement(empty)] = alternate.body.as_slice() else {
            continue;
        };
        let Expression::AssignmentExpression(empty_assignment) =
            empty.expression.without_parentheses()
        else {
            continue;
        };
        if assignment_symbol(model, empty_assignment) != Some(symbol)
            || !matches!(
                empty_assignment.right.without_parentheses(),
                Expression::StringLiteral(literal) if literal.value.is_empty()
            )
        {
            continue;
        }
        if found.is_some() {
            return None;
        }
        found = Some(SourceTextGuard {
            if_end: if_statement.span.end,
            call_start: call.span.start,
        });
    }
    found
}

/// The `fnToString.call(input)` shape of the `e` fallback.
fn source_text_call(
    model: &Model<'_, '_>,
    f: &Facts<'_>,
    site: FunctionSite,
    call: &CallExpression<'_>,
) -> Option<SymbolId> {
    if call.arguments.len() != 1 {
        return None;
    }
    let Expression::StaticMemberExpression(source) = &call.callee else {
        return None;
    };
    if source.property.name != "call" {
        return None;
    }
    let Expression::Identifier(helper) = source.object.without_parentheses() else {
        return None;
    };
    let helper = crate::analysis::identifier_symbol(helper, model.scoping)?;
    if body_writes(model, f, helper) {
        return None;
    }
    let args = call_arguments(call);
    let [input] = args.as_slice() else {
        return None;
    };
    input_at(model, f, site, input, call.span.start).then_some(helper)
}

fn body_writes(model: &Model<'_, '_>, f: &Facts<'_>, symbol: SymbolId) -> bool {
    f.assignments
        .iter()
        .any(|&assignment| assignment_symbol(model, assignment) == Some(symbol))
}

fn has_write_between(
    model: &Model<'_, '_>,
    f: &Facts<'_>,
    symbol: SymbolId,
    from: u32,
    to: u32,
) -> bool {
    f.assignments.iter().any(|&assignment| {
        assignment_symbol(model, assignment) == Some(symbol)
            && assignment.span.start >= from
            && assignment.span.start < to
    })
}

fn assignment_symbol(
    model: &Model<'_, '_>,
    assignment: &AssignmentExpression<'_>,
) -> Option<SymbolId> {
    let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else {
        return None;
    };
    crate::analysis::identifier_symbol(target, model.scoping)
}

fn identifier_is(expression: &Expression<'_>, model: &Model<'_, '_>, symbol: SymbolId) -> bool {
    matches!(
        expression.without_parentheses(),
        Expression::Identifier(identifier)
            if crate::analysis::identifier_symbol(identifier, model.scoping) == Some(symbol)
    )
}

fn enclosing_if<'a>(model: &Model<'_, 'a>, node: NodeId) -> Option<&'a IfStatement<'a>> {
    let mut current = node;
    for _ in 0..64 {
        let parent = model.nodes.parent_id(current);
        if parent == current {
            return None;
        }
        if let AstKind::IfStatement(statement) = model.nodes.kind(parent) {
            return Some(statement);
        }
        current = parent;
    }
    None
}

fn enclosing_try<'a>(model: &Model<'_, 'a>, node: NodeId) -> Option<&'a TryStatement<'a>> {
    let mut current = node;
    for _ in 0..64 {
        let parent = model.nodes.parent_id(current);
        if parent == current {
            return None;
        }
        if let AstKind::TryStatement(statement) = model.nodes.kind(parent) {
            return Some(statement);
        }
        current = parent;
    }
    None
}

fn typeof_function_operand<'a>(expression: &'a Expression<'a>) -> Option<&'a Expression<'a>> {
    let Expression::BinaryExpression(binary) = expression.without_parentheses() else {
        return None;
    };
    if binary.operator.as_str() != "===" && binary.operator.as_str() != "==" {
        return None;
    }
    let unary = match (
        binary.left.without_parentheses(),
        binary.right.without_parentheses(),
    ) {
        (Expression::UnaryExpression(unary), right)
            if unary.operator.as_str() == "typeof" && is_function_string(right) =>
        {
            unary
        }
        (left, Expression::UnaryExpression(unary))
            if unary.operator.as_str() == "typeof" && is_function_string(left) =>
        {
            unary
        }
        _ => return None,
    };
    Some(&unary.argument)
}
fn is_function_string(expression: &Expression<'_>) -> bool {
    matches!(
        expression.without_parentheses(),
        Expression::StringLiteral(literal) if literal.value == "function"
    )
}
/// The beacon report row assembled by an owned inline `.map` callback: the
/// callback's own record local predeclares `beaconEndpoint` from a field of
/// the callback formal, then receives exactly one `beaconData` write from a
/// payload call, and is returned. A read-only endpoint mention, a foreign or
/// captured row, a missing payload write, a rebuilt/overwritten row, a nested
/// donor and a returned payload object all refuse. Only this callback's own
/// statements and facts are inspected; no object-literal key is collected
/// globally and `members` (which includes reads) is never consulted.
fn beacon_literal_endpoint_record(
    model: &Model<'_, '_>,
    f: &Facts<'_>,
    all: &BTreeMap<u32, Facts<'_>>,
) -> bool {
    f.calls
        .iter()
        .filter(|call| static_member_name(&call.callee) == Some("map"))
        .any(|call| {
            call_arguments(call)
                .into_iter()
                .filter(|argument| {
                    matches!(
                        argument.without_parentheses(),
                        Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_)
                    )
                })
                .filter_map(|argument| model.function_from_expression(argument))
                .any(|callback| beacon_literal_endpoint_callback(model, callback, all))
        })
}
fn beacon_literal_endpoint_callback(
    model: &Model<'_, '_>,
    callback: FunctionSite,
    all: &BTreeMap<u32, Facts<'_>>,
) -> bool {
    let Some(facts) = all.get(&callback.span.start) else {
        return false;
    };
    // The callback returns the row local itself, not a payload or donor value.
    let [returned] = facts.returns.as_slice() else {
        return false;
    };
    let Expression::Identifier(returned_identifier) = returned.without_parentheses() else {
        return false;
    };
    let Some(row) = crate::analysis::identifier_symbol(returned_identifier, model.scoping) else {
        return false;
    };
    if !declared_within_body(model, row, callback) {
        return false;
    }
    let Some(body) = callback_body(model, callback) else {
        return false;
    };
    // The row is declared exactly once, as an object literal predeclaring the
    // endpoint field from this callback's own formal row.
    let mut row_start = None;
    for statement in &body.statements {
        let Statement::VariableDeclaration(declaration) = statement else {
            continue;
        };
        for declarator in &declaration.declarations {
            let BindingPattern::BindingIdentifier(identifier) = &declarator.id else {
                continue;
            };
            if identifier.symbol_id.get() != Some(row) {
                continue;
            }
            let Some(initializer) = declarator.init.as_ref() else {
                return false;
            };
            let Expression::ObjectExpression(object) = initializer.without_parentheses() else {
                return false;
            };
            if !literal_endpoint_property(object, model, callback) {
                return false;
            }
            if row_start.replace(declarator.span.start).is_some() {
                return false;
            }
        }
    }
    let Some(row_start) = row_start else {
        return false;
    };
    // The same row object receives one payload serialization call; an endpoint
    // write or a second data write means the record was rebuilt/overwritten.
    let mut payload_writes = 0;
    for assignment in &facts.assignments {
        let AssignmentTarget::StaticMemberExpression(member) = &assignment.left else {
            continue;
        };
        if crate::analysis::expression_symbol(member.object.without_parentheses(), model.scoping)
            != Some(row)
        {
            continue;
        }
        match member.property.name.as_str() {
            "beaconEndpoint" => return false,
            "beaconData" => {
                payload_writes += 1;
                if assignment.span.start <= row_start
                    || !matches!(
                        assignment.right.without_parentheses(),
                        Expression::CallExpression(_)
                    )
                {
                    return false;
                }
            }
            _ => (),
        }
    }
    payload_writes == 1
}
fn literal_endpoint_property(
    object: &ObjectExpression<'_>,
    model: &Model<'_, '_>,
    callback: FunctionSite,
) -> bool {
    let mut endpoint = 0;
    for property in &object.properties {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            return false;
        };
        if property.computed || property.method || property.kind != PropertyKind::Init {
            return false;
        };
        if property_key_name(&property.key).as_deref() != Some("beaconEndpoint") {
            continue;
        }
        endpoint += 1;
        let Expression::StaticMemberExpression(member) = property.value.without_parentheses()
        else {
            return false;
        };
        if model
            .site_parameter_slot(&member.object, callback)
            .is_none()
        {
            return false;
        }
    }
    endpoint == 1
}
/// A sibling beacon row assembled by an owned inline `.map` callback: the
/// callback's own record local predeclares `beaconEndpoint` and
/// `beaconDescriptor` from fields of the callback formal, then receives exactly
/// one `payload` write from a captured encoder invoked on a capture object the
/// callback builds from the same record, and is returned. The caller's
/// filtered-record alias and returned `.map` frame are proven too. A read-only
/// mention, a foreign/captured row or descriptor, a missing/duplicate payload
/// write, a foreign capture, a wrong filter alias, a nested donor and any extra
/// row/capture effect all refuse. Only this callback's own statements and facts
/// are inspected; no object-literal key is collected globally and `members`
/// (which includes reads) is never consulted.
fn beacon_descriptor_payload_record(
    model: &Model<'_, '_>,
    site: FunctionSite,
    f: &Facts<'_>,
    all: &BTreeMap<u32, Facts<'_>>,
) -> bool {
    let [returned] = f.returns.as_slice() else {
        return false;
    };
    let Expression::CallExpression(map_call) = returned.without_parentheses() else {
        return false;
    };
    if static_member_name(&map_call.callee) != Some("map") {
        return false;
    }
    let Expression::StaticMemberExpression(map_member) = &map_call.callee else {
        return false;
    };
    let Some(filtered) = crate::analysis::expression_symbol(&map_member.object, model.scoping)
    else {
        return false;
    };
    if !declared_within_body(model, filtered, site) {
        return false;
    }
    let Some(endpoint_field) = filtered_record_field(model, site, f, filtered) else {
        return false;
    };
    let Some(callback) = call_arguments(map_call)
        .into_iter()
        .find_map(|argument| model.function_from_expression(argument))
    else {
        return false;
    };
    beacon_descriptor_payload_callback(model, callback, all, &endpoint_field)
}
/// The `.map` receiver is this function's own filtered-record alias: exactly
/// one `.filter` call on a captured record list, whose result is the receiver's
/// final write in source order (so a later raw-list override refuses), and an
/// inline predicate that reads one field of its own formal and compares it to
/// this function's formal (directly or through a one-step local alias). The
/// predicate may be a concise arrow or an ordinary single-return body. The
/// predicate field is returned so the callback's endpoint field must be the same
/// filtered field.
fn filtered_record_field(
    model: &Model<'_, '_>,
    site: FunctionSite,
    f: &Facts<'_>,
    filtered: SymbolId,
) -> Option<String> {
    let mut filter_calls = f
        .calls
        .iter()
        .filter(|call| static_member_name(&call.callee) == Some("filter"));
    let filter_call = *filter_calls.next()?;
    if filter_calls.next().is_some() {
        return None;
    }
    let Expression::StaticMemberExpression(member) = &filter_call.callee else {
        return None;
    };
    let records = crate::analysis::expression_symbol(&member.object, model.scoping)?;
    if declared_within_body(model, records, site) {
        return None;
    }
    if model
        .formal_symbol_slot(records)
        .is_some_and(|slot| slot.owner == site.node_id)
    {
        return None;
    }
    // The returned `.map` receiver must still hold this filter's result. It may
    // be pre-assigned from the captured record list, but the filter call has to
    // be its final write in source order; a later raw-list override (or any
    // other write) discards the filter and refuses.
    let mut filter_write = None;
    for &assignment in &f.assignments {
        if assignment_symbol(model, assignment) != Some(filtered) {
            continue;
        }
        if assignment.operator != AssignmentOperator::Assign {
            return None;
        }
        let right = assignment.right.without_parentheses();
        let from_filter = matches!(
            right,
            Expression::CallExpression(call) if call.span.start == filter_call.span.start
        );
        let from_records = matches!(
            right,
            Expression::Identifier(identifier)
                if crate::analysis::identifier_symbol(identifier, model.scoping) == Some(records)
        );
        if !from_filter && !from_records {
            return None;
        }
        if from_filter {
            filter_write = Some(assignment);
        }
    }
    let filter_write = filter_write?;
    if f.assignments.iter().any(|&assignment| {
        assignment_symbol(model, assignment) == Some(filtered)
            && assignment.span.start > filter_write.span.start
    }) {
        return None;
    }
    let predicate = call_arguments(filter_call)
        .into_iter()
        .find_map(|argument| model.function_from_expression(argument))?;
    let returned = callback_return_expression(model, predicate)?;
    let Expression::BinaryExpression(binary) = returned.without_parentheses() else {
        return None;
    };
    if binary.operator.as_str() != "===" {
        return None;
    }
    let (field, other) = if let Some(field) = formal_member_field(model, predicate, &binary.left) {
        (field, &binary.right)
    } else {
        (
            formal_member_field(model, predicate, &binary.right)?,
            &binary.left,
        )
    };
    target_formal_alias(model, site, other).then_some(field)
}
fn formal_member_field(
    model: &Model<'_, '_>,
    site: FunctionSite,
    expression: &Expression<'_>,
) -> Option<String> {
    let Expression::StaticMemberExpression(member) = expression.without_parentheses() else {
        return None;
    };
    model.site_parameter_slot(&member.object, site)?;
    Some(member.property.name.to_string())
}
fn target_formal_alias(
    model: &Model<'_, '_>,
    site: FunctionSite,
    expression: &Expression<'_>,
) -> bool {
    let Some(symbol) =
        crate::analysis::expression_symbol(expression.without_parentheses(), model.scoping)
    else {
        return false;
    };
    if model
        .formal_symbol_slot(symbol)
        .is_some_and(|slot| slot.owner == site.node_id)
    {
        return true;
    }
    if !declared_within_body(model, symbol, site) || model.binding_writes(symbol) != 0 {
        return false;
    }
    match model.initializer(symbol) {
        Some(Expression::Identifier(identifier)) => {
            crate::analysis::identifier_symbol(identifier, model.scoping).is_some_and(|inner| {
                model
                    .formal_symbol_slot(inner)
                    .is_some_and(|slot| slot.owner == site.node_id)
            })
        }
        _ => false,
    }
}
fn beacon_descriptor_payload_callback(
    model: &Model<'_, '_>,
    callback: FunctionSite,
    all: &BTreeMap<u32, Facts<'_>>,
    endpoint_field: &str,
) -> bool {
    let Some(facts) = all.get(&callback.span.start) else {
        return false;
    };
    // The callback returns its own row local, not a payload or donor value.
    let [returned] = facts.returns.as_slice() else {
        return false;
    };
    let Expression::Identifier(returned_identifier) = returned.without_parentheses() else {
        return false;
    };
    let Some(row) = crate::analysis::identifier_symbol(returned_identifier, model.scoping) else {
        return false;
    };
    if !declared_within_body(model, row, callback) {
        return false;
    }
    let Some(body) = callback_body(model, callback) else {
        return false;
    };
    let Some((row_start, object)) = declared_object(body, row) else {
        return false;
    };
    let Some((endpoint, descriptor)) = literal_descriptor_row(object, model, callback) else {
        return false;
    };
    if endpoint != endpoint_field {
        return false;
    }
    let Some(payload_call) = payload_write(model, facts, row, row_start) else {
        return false;
    };
    // The payload is one call through a captured binding, with the capture
    // object as its sole argument. A local encoder or a borrowed value refuses.
    let Expression::Identifier(encoder) = &payload_call.callee else {
        return false;
    };
    let Some(encoder_symbol) = crate::analysis::identifier_symbol(encoder, model.scoping) else {
        return false;
    };
    if declared_within_body(model, encoder_symbol, callback) {
        return false;
    }
    let arguments = call_arguments(payload_call);
    let [argument] = arguments.as_slice() else {
        return false;
    };
    let Expression::Identifier(capture_identifier) = argument.without_parentheses() else {
        return false;
    };
    let Some(capture) = crate::analysis::identifier_symbol(capture_identifier, model.scoping)
    else {
        return false;
    };
    if capture == row || !declared_within_body(model, capture, callback) {
        return false;
    }
    descriptor_capture(model, callback, facts, capture, &descriptor)
}
fn declared_object<'a>(
    body: &'a FunctionBody<'a>,
    symbol: SymbolId,
) -> Option<(u32, &'a ObjectExpression<'a>)> {
    let mut found = None;
    for statement in &body.statements {
        let Statement::VariableDeclaration(declaration) = statement else {
            continue;
        };
        for declarator in &declaration.declarations {
            let BindingPattern::BindingIdentifier(identifier) = &declarator.id else {
                continue;
            };
            if identifier.symbol_id.get() != Some(symbol) {
                continue;
            }
            let Some(Expression::ObjectExpression(object)) = declarator
                .init
                .as_ref()
                .map(|init| init.without_parentheses())
            else {
                return None;
            };
            if found.replace((declarator.span.start, &**object)).is_some() {
                return None;
            }
        }
    }
    found
}
/// The row literal has the two beacon descriptor fields, both read from
/// this callback's own formal; the endpoint field is returned for the filtered
/// record join and the descriptor field for the capture join. An optional inert
/// `payload: void 0` reserves the field before the separately proven sole write.
fn literal_descriptor_row(
    object: &ObjectExpression<'_>,
    model: &Model<'_, '_>,
    callback: FunctionSite,
) -> Option<(String, String)> {
    let mut endpoint = None;
    let mut descriptor = None;
    let mut placeholder = false;
    for property in &object.properties {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            return None;
        };
        if property.computed || property.method || property.kind != PropertyKind::Init {
            return None;
        }
        let key = property_key_name(&property.key)?;
        if key == "payload" {
            if placeholder || !descriptor_void_zero(&property.value) {
                return None;
            }
            placeholder = true;
            continue;
        }
        let Expression::StaticMemberExpression(member) = property.value.without_parentheses()
        else {
            return None;
        };
        if model
            .site_parameter_slot(&member.object, callback)
            .is_none()
        {
            return None;
        }
        let field = member.property.name.to_string();
        match key.as_str() {
            "beaconEndpoint" => {
                if endpoint.replace(field).is_some() {
                    return None;
                }
            }
            "beaconDescriptor" => {
                if descriptor.replace(field).is_some() {
                    return None;
                }
            }
            _ => return None,
        }
    }
    Some((endpoint?, descriptor?))
}
/// Literal void-zero only: no shadowable undefined binding or operand effects.
fn descriptor_void_zero(expression: &Expression<'_>) -> bool {
    matches!(expression.without_parentheses(), Expression::UnaryExpression(unary)
        if unary.operator.as_str() == "void"
            && matches!(unary.argument.without_parentheses(), Expression::NumericLiteral(n) if n.value == 0.0))
}
/// Exactly one write on the row, after its declaration: `payload` assigned a
/// call. Any endpoint/descriptor/data write, a second payload write, an earlier
/// write or any other row effect refuses.
fn payload_write<'a>(
    model: &Model<'_, '_>,
    facts: &Facts<'a>,
    row: SymbolId,
    row_start: u32,
) -> Option<&'a CallExpression<'a>> {
    let mut payload = None;
    let mut row_writes = 0;
    for &assignment in &facts.assignments {
        let AssignmentTarget::StaticMemberExpression(member) = &assignment.left else {
            continue;
        };
        if crate::analysis::expression_symbol(&member.object, model.scoping) != Some(row) {
            continue;
        }
        row_writes += 1;
        if member.property.name.as_str() != "payload" || assignment.span.start <= row_start {
            return None;
        }
        let Expression::CallExpression(call) = assignment.right.without_parentheses() else {
            return None;
        };
        if payload.replace(call).is_some() {
            return None;
        }
    }
    if row_writes != 1 {
        return None;
    }
    payload.map(|v| &**v)
}
/// The payload argument is the callback's own capture object: exactly a
/// `signalFilter` formal read and a literal `notifyLastChance: true`, then
/// exactly one `encode` call (the captured encoder over a formal-record code
/// list) and one `descriptor` formal read. Any missing, duplicate, foreign or
/// extra capture effect refuses.
fn descriptor_capture(
    model: &Model<'_, '_>,
    callback: FunctionSite,
    facts: &Facts<'_>,
    capture: SymbolId,
    descriptor_field: &str,
) -> bool {
    let Some(body) = callback_body(model, callback) else {
        return false;
    };
    let Some((capture_start, object)) = declared_object(body, capture) else {
        return false;
    };
    let mut signal = false;
    let mut notify = false;
    for property in &object.properties {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            return false;
        };
        if property.computed || property.method || property.kind != PropertyKind::Init {
            return false;
        }
        let Some(key) = property_key_name(&property.key) else {
            return false;
        };
        match key.as_str() {
            "signalFilter" => {
                if signal {
                    return false;
                }
                let Expression::StaticMemberExpression(member) =
                    property.value.without_parentheses()
                else {
                    return false;
                };
                if model
                    .site_parameter_slot(&member.object, callback)
                    .is_none()
                {
                    return false;
                }
                signal = true;
            }
            "notifyLastChance" => {
                if notify {
                    return false;
                }
                if !matches!(
                    property.value.without_parentheses(),
                    Expression::BooleanLiteral(literal) if literal.value
                ) {
                    return false;
                }
                notify = true;
            }
            _ => return false,
        }
    }
    if !signal || !notify {
        return false;
    }
    let mut encode = false;
    let mut descriptor = false;
    let mut writes = 0;
    for &assignment in &facts.assignments {
        let AssignmentTarget::StaticMemberExpression(member) = &assignment.left else {
            continue;
        };
        if crate::analysis::expression_symbol(&member.object, model.scoping) != Some(capture) {
            continue;
        }
        writes += 1;
        if assignment.span.start <= capture_start {
            return false;
        }
        match member.property.name.as_str() {
            "encode" => {
                if encode || !descriptor_encoder_invocation(model, callback, &assignment.right) {
                    return false;
                }
                encode = true;
            }
            "descriptor" => {
                if descriptor {
                    return false;
                }
                let Expression::StaticMemberExpression(member) =
                    assignment.right.without_parentheses()
                else {
                    return false;
                };
                if model
                    .site_parameter_slot(&member.object, callback)
                    .is_none()
                    || member.property.name.as_str() != descriptor_field
                {
                    return false;
                }
                descriptor = true;
            }
            _ => return false,
        }
    }
    writes == 2 && encode && descriptor
}
fn descriptor_encoder_invocation(
    model: &Model<'_, '_>,
    callback: FunctionSite,
    expression: &Expression<'_>,
) -> bool {
    let call = match expression.without_parentheses() {
        Expression::CallExpression(call) => call,
        Expression::Identifier(identifier) => {
            let Some(symbol) = crate::analysis::identifier_symbol(identifier, model.scoping) else {
                return false;
            };
            if !declared_within_body(model, symbol, callback) || model.binding_writes(symbol) != 0 {
                return false;
            }
            let Some(initializer) = model.initializer(symbol) else {
                return false;
            };
            let Expression::CallExpression(call) = initializer.without_parentheses() else {
                return false;
            };
            call
        }
        _ => return false,
    };
    let Expression::Identifier(callee) = &call.callee else {
        return false;
    };
    let Some(encoder) = crate::analysis::identifier_symbol(callee, model.scoping) else {
        return false;
    };
    if declared_within_body(model, encoder, callback) {
        return false;
    }
    call_arguments(call).into_iter().any(|argument| {
        let Expression::CallExpression(inner) = argument.without_parentheses() else {
            return false;
        };
        if static_member_name(&inner.callee) != Some("map") {
            return false;
        }
        let Expression::StaticMemberExpression(member) = &inner.callee else {
            return false;
        };
        // The code list is a field of this callback's own formal record, e.g.
        // `record.c.map(...)`; the member object is therefore a member of the
        // formal, not the formal itself.
        let Expression::StaticMemberExpression(record_member) = member.object.without_parentheses()
        else {
            return false;
        };
        model
            .site_parameter_slot(&record_member.object, callback)
            .is_some()
            && call_arguments(inner)
                .into_iter()
                .any(|argument| model.function_from_expression(argument).is_some())
    })
}
fn callback_body<'a>(
    model: &Model<'_, 'a>,
    callback: FunctionSite,
) -> Option<&'a FunctionBody<'a>> {
    match model.nodes.kind(callback.node_id) {
        AstKind::Function(function) => function.body.as_deref(),
        AstKind::ArrowFunctionExpression(arrow) => Some(&*arrow.body),
        _ => None,
    }
}
/// The single returned expression of an inline callback: an ordinary
/// one-`return` body or a concise arrow's sole expression statement. Only this
/// callback's own body statements are inspected; returns from nested closures
/// are never borrowed.
fn callback_return_expression<'a>(
    model: &Model<'_, 'a>,
    callback: FunctionSite,
) -> Option<&'a Expression<'a>> {
    let body = callback_body(model, callback)?;
    if !body.directives.is_empty() {
        return None;
    }
    match model.nodes.kind(callback.node_id) {
        AstKind::ArrowFunctionExpression(arrow) if arrow.expression => {
            match body.statements.as_slice() {
                [Statement::ExpressionStatement(statement)] => Some(&statement.expression),
                _ => None,
            }
        }
        _ => match body.statements.as_slice() {
            [Statement::ReturnStatement(statement)] => statement.argument.as_ref(),
            _ => None,
        },
    }
}
fn declared_within_body(model: &Model<'_, '_>, symbol: SymbolId, site: FunctionSite) -> bool {
    let body = site.body_span.unwrap_or(site.span);
    let declaration = model.binding_ref(symbol).declaration;
    declaration.start >= body.start && declaration.end <= body.end
}
/// The three owned bodies of the global-collection API carrier: the
/// four-formal outer factory that reads `ctx.global` and returns its own
/// `{ init, teardown }` pair, the zero-formal initialiser that saves the
/// protocol `ns_collect` slot and installs an owned encoded-header collector
/// (plus, on a captured dynamic key, an owned descriptor-payload mapper), and
/// the encoded-header collector itself. Every identity is a direct scoped
/// symbol, an exact formal slot, a captured callee or an exact loop/local
/// statement; no generated spelling, vendor, prefix literal, position or hash
/// is read. The initialiser reuses the actual accepted descriptor-payload proof
/// on the actual nested mapper function, so the pair is bound to that real body
/// and not to an inferred label.
#[derive(Clone, Copy)]
#[allow(dead_code)]
struct GcCollectionInit {
    target: SymbolId,
    key: SymbolId,
    saved_collector: SymbolId,
    saved_mapper: SymbolId,
    collector: FunctionSite,
    mapper: FunctionSite,
}
fn gc_symbol(model: &Model<'_, '_>, expression: &Expression<'_>) -> Option<SymbolId> {
    crate::analysis::expression_symbol(expression.without_parentheses(), model.scoping)
}
fn gc_is_captured_symbol(model: &Model<'_, '_>, site: FunctionSite, symbol: SymbolId) -> bool {
    !declared_within_body(model, symbol, site) && model.formal_symbol_slot(symbol).is_none()
}
fn gc_captured(
    model: &Model<'_, '_>,
    site: FunctionSite,
    expression: &Expression<'_>,
) -> Option<SymbolId> {
    let symbol = gc_symbol(model, expression)?;
    gc_is_captured_symbol(model, site, symbol).then_some(symbol)
}
fn gc_is_unset(expression: &Expression<'_>) -> bool {
    match expression.without_parentheses() {
        Expression::NullLiteral(_) => true,
        Expression::UnaryExpression(unary) => unary.operator.as_str() == "void",
        Expression::Identifier(identifier) => identifier.name.as_str() == "undefined",
        _ => false,
    }
}
fn gc_block<'a>(statement: &'a Statement<'a>) -> Option<&'a [Statement<'a>]> {
    match statement {
        Statement::BlockStatement(block) => Some(block.body.as_slice()),
        _ => None,
    }
}
fn gc_plain_assignment<'a>(statement: &'a Statement<'a>) -> Option<&'a AssignmentExpression<'a>> {
    let Statement::ExpressionStatement(expression) = statement else {
        return None;
    };
    let Expression::AssignmentExpression(assignment) = &expression.expression else {
        return None;
    };
    (assignment.operator.as_str() == "=").then_some(assignment)
}
fn gc_single_declarator<'a>(
    statement: &'a Statement<'a>,
) -> Option<(&'a BindingPattern<'a>, Option<&'a Expression<'a>>)> {
    let Statement::VariableDeclaration(declaration) = statement else {
        return None;
    };
    let [declarator] = declaration.declarations.as_slice() else {
        return None;
    };
    Some((&declarator.id, declarator.init.as_ref()))
}
fn gc_declarator_symbol(pattern: &BindingPattern<'_>) -> Option<SymbolId> {
    let BindingPattern::BindingIdentifier(identifier) = pattern else {
        return None;
    };
    identifier.symbol_id.get()
}
fn gc_member_matches(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    receiver: SymbolId,
    named: Option<&str>,
    key: Option<SymbolId>,
) -> bool {
    match expression.without_parentheses() {
        Expression::StaticMemberExpression(member) if !member.optional => {
            named.is_some_and(|name| member.property.name.as_str() == name)
                && gc_symbol(model, &member.object) == Some(receiver)
        }
        Expression::ComputedMemberExpression(member) if !member.optional => {
            gc_symbol(model, &member.object) == Some(receiver)
                && key.is_some_and(|key| gc_symbol(model, &member.expression) == Some(key))
        }
        _ => false,
    }
}
fn gc_target_matches(
    model: &Model<'_, '_>,
    target: &AssignmentTarget<'_>,
    receiver: SymbolId,
    named: Option<&str>,
    key: Option<SymbolId>,
) -> bool {
    match target {
        AssignmentTarget::StaticMemberExpression(member) if !member.optional => {
            named.is_some_and(|name| member.property.name.as_str() == name)
                && gc_symbol(model, &member.object) == Some(receiver)
        }
        AssignmentTarget::ComputedMemberExpression(member) if !member.optional => {
            gc_symbol(model, &member.object) == Some(receiver)
                && key.is_some_and(|key| gc_symbol(model, &member.expression) == Some(key))
        }
        _ => false,
    }
}
fn gc_object_create_null(model: &Model<'_, '_>, expression: &Expression<'_>) -> bool {
    let Expression::CallExpression(call) = expression.without_parentheses() else {
        return false;
    };
    if call.optional || expression_path(model, &call.callee) != ["Object", "create"] {
        return false;
    }
    let arguments = call_arguments(call);
    matches!(arguments.as_slice(), [argument]
        if matches!(argument.without_parentheses(), Expression::NullLiteral(_)))
}
fn gc_save_named(
    model: &Model<'_, '_>,
    statement: &Statement<'_>,
    name: &str,
) -> Option<(SymbolId, SymbolId)> {
    let assignment = gc_plain_assignment(statement)?;
    let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left else {
        return None;
    };
    let saved = crate::analysis::identifier_symbol(identifier, model.scoping)?;
    let Expression::StaticMemberExpression(member) = assignment.right.without_parentheses() else {
        return None;
    };
    if member.optional || member.property.name.as_str() != name {
        return None;
    }
    let receiver = gc_symbol(model, &member.object)?;
    Some((receiver, saved))
}
fn gc_install_named(
    model: &Model<'_, '_>,
    statement: &Statement<'_>,
    name: &str,
) -> Option<(SymbolId, SymbolId)> {
    let assignment = gc_plain_assignment(statement)?;
    let AssignmentTarget::StaticMemberExpression(member) = &assignment.left else {
        return None;
    };
    if member.optional || member.property.name.as_str() != name {
        return None;
    }
    let receiver = gc_symbol(model, &member.object)?;
    let installed = gc_symbol(model, &assignment.right)?;
    Some((receiver, installed))
}
fn gc_save_computed(
    model: &Model<'_, '_>,
    statement: &Statement<'_>,
) -> Option<(SymbolId, SymbolId, SymbolId)> {
    let assignment = gc_plain_assignment(statement)?;
    let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left else {
        return None;
    };
    let saved = crate::analysis::identifier_symbol(identifier, model.scoping)?;
    let Expression::ComputedMemberExpression(member) = assignment.right.without_parentheses()
    else {
        return None;
    };
    if member.optional {
        return None;
    }
    let receiver = gc_symbol(model, &member.object)?;
    let key = gc_symbol(model, &member.expression)?;
    Some((receiver, key, saved))
}
fn gc_install_computed(
    model: &Model<'_, '_>,
    statement: &Statement<'_>,
) -> Option<(SymbolId, SymbolId, SymbolId)> {
    let assignment = gc_plain_assignment(statement)?;
    let AssignmentTarget::ComputedMemberExpression(member) = &assignment.left else {
        return None;
    };
    if member.optional {
        return None;
    }
    let receiver = gc_symbol(model, &member.object)?;
    let key = gc_symbol(model, &member.expression)?;
    let installed = gc_symbol(model, &assignment.right)?;
    Some((receiver, key, installed))
}
fn gc_function_symbol(
    model: &Model<'_, '_>,
    statement: &Statement<'_>,
) -> Option<(SymbolId, FunctionSite)> {
    let Statement::FunctionDeclaration(function) = statement else {
        return None;
    };
    let symbol = function.id.as_ref()?.symbol_id.get()?;
    let site = model.function_from_symbol(symbol)?;
    Some((symbol, site))
}
fn gc_return_symbol(model: &Model<'_, '_>, statement: &Statement<'_>) -> Option<SymbolId> {
    let Statement::ReturnStatement(statement) = statement else {
        return None;
    };
    gc_symbol(model, statement.argument.as_ref()?)
}
fn gc_returns_void(statement: &Statement<'_>) -> bool {
    let Statement::ReturnStatement(statement) = statement else {
        return false;
    };
    let Some(argument) = statement.argument.as_ref() else {
        return false;
    };
    matches!(
        argument.without_parentheses(),
        Expression::UnaryExpression(unary) if unary.operator.as_str() == "void"
    )
}
fn gc_restores(
    model: &Model<'_, '_>,
    statement: &Statement<'_>,
    receiver: SymbolId,
    named: Option<&str>,
    key: Option<SymbolId>,
    saved: SymbolId,
) -> bool {
    let Statement::IfStatement(branch) = statement else {
        return false;
    };
    if branch.alternate.is_some() {
        return false;
    }
    let Expression::BinaryExpression(binary) = branch.test.without_parentheses() else {
        return false;
    };
    if !matches!(binary.operator.as_str(), "!==" | "!=")
        || gc_symbol(model, &binary.right) != Some(saved)
        || !gc_member_matches(model, &binary.left, receiver, named, key)
    {
        return false;
    }
    let Some(body) = gc_block(&branch.consequent) else {
        return false;
    };
    let [inner] = body else {
        return false;
    };
    let Some(assignment) = gc_plain_assignment(inner) else {
        return false;
    };
    gc_symbol(model, &assignment.right) == Some(saved)
        && gc_target_matches(model, &assignment.left, receiver, named, key)
}
fn gc_restore_pair(
    model: &Model<'_, '_>,
    site: FunctionSite,
    target: SymbolId,
    key: SymbolId,
    saved_collector: SymbolId,
    saved_mapper: SymbolId,
) -> bool {
    let Some(body) = callback_body(model, site) else {
        return false;
    };
    if !body.directives.is_empty() {
        return false;
    }
    let [first, second] = body.statements.as_slice() else {
        return false;
    };
    gc_restores(
        model,
        first,
        target,
        Some("ns_collect"),
        None,
        saved_collector,
    ) && gc_restores(model, second, target, None, Some(key), saved_mapper)
}
fn gc_bound_void_call(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statement: &Statement<'_>,
) -> Option<(SymbolId, SymbolId)> {
    let (pattern, init) = gc_single_declarator(statement)?;
    let bound = gc_declarator_symbol(pattern)?;
    let Expression::CallExpression(call) = init?.without_parentheses() else {
        return None;
    };
    if call.optional {
        return None;
    }
    let callee = gc_captured(model, site, &call.callee)?;
    let arguments = call_arguments(call);
    let [argument] = arguments.as_slice() else {
        return None;
    };
    gc_is_unset(argument).then_some((bound, callee))
}
fn gc_bound_object_create_null(
    model: &Model<'_, '_>,
    statement: &Statement<'_>,
) -> Option<SymbolId> {
    let (pattern, init) = gc_single_declarator(statement)?;
    let bound = gc_declarator_symbol(pattern)?;
    gc_object_create_null(model, init?).then_some(bound)
}
fn gc_uninitialized_binding(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statement: &Statement<'_>,
) -> Option<SymbolId> {
    let (pattern, init) = gc_single_declarator(statement)?;
    if init.is_some() {
        return None;
    }
    let symbol = gc_declarator_symbol(pattern)?;
    declared_within_body(model, symbol, site).then_some(symbol)
}
fn gc_for_in<'a>(
    statement: &'a Statement<'a>,
) -> Option<(SymbolId, &'a Expression<'a>, &'a Statement<'a>)> {
    let Statement::ForInStatement(loop_) = statement else {
        return None;
    };
    let ForStatementLeft::VariableDeclaration(declaration) = &loop_.left else {
        return None;
    };
    let [declarator] = declaration.declarations.as_slice() else {
        return None;
    };
    if declarator.init.is_some() {
        return None;
    }
    let BindingPattern::BindingIdentifier(identifier) = &declarator.id else {
        return None;
    };
    let key = identifier.symbol_id.get()?;
    Some((key, &loop_.right, &loop_.body))
}
fn gc_not_unset(model: &Model<'_, '_>, expression: &Expression<'_>, symbol: SymbolId) -> bool {
    let Expression::BinaryExpression(binary) = expression.without_parentheses() else {
        return false;
    };
    if !matches!(binary.operator.as_str(), "!=" | "!==") {
        return false;
    }
    (gc_symbol(model, &binary.left) == Some(symbol) && gc_is_unset(&binary.right))
        || (gc_symbol(model, &binary.right) == Some(symbol) && gc_is_unset(&binary.left))
}
fn gc_not_unset_pair(model: &Model<'_, '_>, expression: &Expression<'_>, symbol: SymbolId) -> bool {
    let Expression::LogicalExpression(logical) = expression.without_parentheses() else {
        return false;
    };
    logical.operator.as_str() == "&&"
        && gc_not_unset(model, &logical.left, symbol)
        && gc_not_unset(model, &logical.right, symbol)
}
fn gc_member_not_unset(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    receiver: SymbolId,
    name: &str,
) -> bool {
    let Expression::BinaryExpression(binary) = expression.without_parentheses() else {
        return false;
    };
    matches!(binary.operator.as_str(), "!=" | "!==")
        && gc_member_matches(model, &binary.left, receiver, Some(name), None)
        && gc_is_unset(&binary.right)
}
fn gc_null_check(model: &Model<'_, '_>, expression: &Expression<'_>, input: SymbolId) -> bool {
    let Expression::BinaryExpression(binary) = expression.without_parentheses() else {
        return false;
    };
    matches!(binary.operator.as_str(), "==" | "===")
        && ((gc_symbol(model, &binary.left) == Some(input) && gc_is_unset(&binary.right))
            || (gc_is_unset(&binary.left) && gc_symbol(model, &binary.right) == Some(input)))
}
fn gc_policy_call(
    model: &Model<'_, '_>,
    site: FunctionSite,
    expression: &Expression<'_>,
    input: SymbolId,
) -> bool {
    let Expression::CallExpression(call) = expression.without_parentheses() else {
        return false;
    };
    if call.optional || gc_captured(model, site, &call.callee).is_none() {
        return false;
    }
    let arguments = call_arguments(call);
    matches!(arguments.as_slice(), [argument] if gc_symbol(model, argument) == Some(input))
}
fn gc_and_leaves<'a>(expression: &'a Expression<'a>, leaves: &mut Vec<&'a Expression<'a>>) {
    if let Expression::LogicalExpression(logical) = expression.without_parentheses() {
        if logical.operator.as_str() == "&&" {
            gc_and_leaves(&logical.left, leaves);
            gc_and_leaves(&logical.right, leaves);
            return;
        }
    }
    leaves.push(expression.without_parentheses());
}
fn gc_request_guard(
    model: &Model<'_, '_>,
    site: FunctionSite,
    expression: &Expression<'_>,
    input: SymbolId,
) -> bool {
    let Expression::LogicalExpression(or) = expression.without_parentheses() else {
        return false;
    };
    if or.operator.as_str() != "||" || !gc_null_check(model, &or.left, input) {
        return false;
    }
    let mut leaves = Vec::new();
    gc_and_leaves(&or.right, &mut leaves);
    if leaves.len() != 3 {
        return false;
    }
    let mut url = false;
    let mut method = false;
    let mut policy = false;
    for leaf in leaves {
        if gc_member_not_unset(model, leaf, input, "url") {
            url = true;
        } else if gc_member_not_unset(model, leaf, input, "method") {
            method = true;
        } else if gc_policy_call(model, site, leaf, input) {
            policy = true;
        }
    }
    url && method && policy
}
fn gc_own_property_call<'a>(
    model: &Model<'_, '_>,
    expression: &'a Expression<'a>,
) -> Option<(&'a Expression<'a>, &'a Expression<'a>)> {
    let Expression::CallExpression(call) = expression.without_parentheses() else {
        return None;
    };
    if call.optional
        || expression_path(model, &call.callee) != ["Object", "prototype", "hasOwnProperty", "call"]
    {
        return None;
    }
    let arguments = call_arguments(call);
    let [object, key] = arguments.as_slice() else {
        return None;
    };
    Some((*object, *key))
}
fn gc_strip_bangs<'a>(expression: &'a Expression<'a>) -> &'a Expression<'a> {
    let mut current = expression.without_parentheses();
    while let Expression::UnaryExpression(unary) = current {
        if unary.operator.as_str() != "!" {
            break;
        }
        current = unary.argument.without_parentheses();
    }
    current
}
fn gc_computed_target(
    model: &Model<'_, '_>,
    target: &AssignmentTarget<'_>,
    receiver: SymbolId,
    key: SymbolId,
) -> bool {
    matches!(
        target,
        AssignmentTarget::ComputedMemberExpression(member)
            if !member.optional
                && gc_symbol(model, &member.object) == Some(receiver)
                && gc_symbol(model, &member.expression) == Some(key)
    )
}
fn gc_computed_read(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    receiver: SymbolId,
    key: SymbolId,
) -> bool {
    matches!(
        expression.without_parentheses(),
        Expression::ComputedMemberExpression(member)
            if !member.optional
                && gc_symbol(model, &member.object) == Some(receiver)
                && gc_symbol(model, &member.expression) == Some(key)
    )
}
fn gc_formatter_call(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statement: &Statement<'_>,
    loop_key: SymbolId,
    payload: SymbolId,
) -> Option<(SymbolId, SymbolId)> {
    let (pattern, init) = gc_single_declarator(statement)?;
    let formatted = gc_declarator_symbol(pattern)?;
    let Expression::CallExpression(call) = init?.without_parentheses() else {
        return None;
    };
    if call.optional {
        return None;
    }
    let formatter = gc_captured(model, site, &call.callee)?;
    let arguments = call_arguments(call);
    let [prefixed_key, value, limit] = arguments.as_slice() else {
        return None;
    };
    let Expression::BinaryExpression(prefix) = prefixed_key.without_parentheses() else {
        return None;
    };
    if prefix.operator.as_str() != "+"
        || !matches!(
            prefix.left.without_parentheses(),
            Expression::StringLiteral(_)
        )
        || gc_symbol(model, &prefix.right) != Some(loop_key)
    {
        return None;
    }
    if !matches!(
        value.without_parentheses(),
        Expression::ComputedMemberExpression(member)
            if !member.optional
                && gc_symbol(model, &member.object) == Some(payload)
                && gc_symbol(model, &member.expression) == Some(loop_key)
    ) {
        return None;
    }
    if !matches!(limit.without_parentheses(), Expression::NumericLiteral(_)) {
        return None;
    }
    Some((formatted, formatter))
}
fn collect_encoded_headers(model: &Model<'_, '_>, site: FunctionSite) -> bool {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return false;
    };
    if function.r#async || function.generator || function.params.rest.is_some() {
        return false;
    }
    let [parameter] = function.params.items.as_slice() else {
        return false;
    };
    if parameter.initializer.is_some() {
        return false;
    }
    let BindingPattern::BindingIdentifier(input_identifier) = &parameter.pattern else {
        return false;
    };
    let Some(input) = input_identifier.symbol_id.get() else {
        return false;
    };
    if model.binding_writes(input) != 0 || !model.scoping.symbol_redeclarations(input).is_empty() {
        return false;
    }
    let Some(body) = function.body.as_deref() else {
        return false;
    };
    if !body.directives.is_empty() {
        return false;
    }
    let guard = match body.statements.as_slice() {
        [Statement::IfStatement(guard)] if gc_request_guard(model, site, &guard.test, input) => {
            &**guard
        }
        statements => {
            let Some(guard) = encoding::encoded_header_guard::guard(model, site, statements, input)
            else {
                return false;
            };
            guard
        }
    };
    let Some(accepted) = gc_block(&guard.consequent) else {
        return false;
    };
    let Some(rejected_statement) = guard.alternate.as_ref() else {
        return false;
    };
    let Some(rejected) = gc_block(rejected_statement) else {
        return false;
    };
    let [Statement::ReturnStatement(rejected_return)] = rejected else {
        return false;
    };
    let Some(rejected_argument) = rejected_return.argument.as_ref() else {
        return false;
    };
    if !matches!(
        rejected_argument.without_parentheses(),
        Expression::NullLiteral(_)
    ) {
        return false;
    }
    let [generator_statement, dictionary_statement, copy_guard_statement, accepted_return] =
        accepted
    else {
        return false;
    };
    let Some((payload, generator)) = gc_bound_void_call(model, site, generator_statement) else {
        return false;
    };
    let Some(dictionary) = gc_bound_object_create_null(model, dictionary_statement) else {
        return false;
    };
    if gc_return_symbol(model, accepted_return) != Some(dictionary) {
        return false;
    }
    let Statement::IfStatement(copy_guard) = copy_guard_statement else {
        return false;
    };
    if copy_guard.alternate.is_some() || !gc_not_unset_pair(model, &copy_guard.test, payload) {
        return false;
    }
    let Some(copy_body) = gc_block(&copy_guard.consequent) else {
        return false;
    };
    let [last_key_statement, entries_loop] = copy_body else {
        return false;
    };
    // This local is overwritten by the owned inner loop before every read.
    // Accept the explicit inert initializer as well as the omitted one; keep
    // the carrier's uninitialized-state proof separate.
    let Some(last_key) = gc_header_key_binding(model, site, last_key_statement) else {
        return false;
    };
    let Some((loop_key, loop_source, loop_body)) = gc_for_in(entries_loop) else {
        return false;
    };
    if gc_symbol(model, loop_source) != Some(payload) {
        return false;
    }
    let Some(loop_statements) = gc_block(loop_body) else {
        return false;
    };
    let [own_guard_statement] = loop_statements else {
        return false;
    };
    let Statement::IfStatement(own_guard) = own_guard_statement else {
        return false;
    };
    if own_guard.alternate.is_some() {
        return false;
    }
    let Some((own_object, own_key)) = gc_own_property_call(model, &own_guard.test) else {
        return false;
    };
    if gc_symbol(model, own_object) != Some(payload) || gc_symbol(model, own_key) != Some(loop_key)
    {
        return false;
    }
    let Some(own_body) = gc_block(&own_guard.consequent) else {
        return false;
    };
    let [formatter_statement, formatted_guard_statement] = own_body else {
        return false;
    };
    let Some((formatted, formatter)) =
        gc_formatter_call(model, site, formatter_statement, loop_key, payload)
    else {
        return false;
    };
    if formatter == generator {
        return false;
    }
    let Statement::IfStatement(formatted_guard) = formatted_guard_statement else {
        return false;
    };
    if formatted_guard.alternate.is_some()
        || !gc_not_unset_pair(model, &formatted_guard.test, formatted)
    {
        return false;
    }
    let Some(formatted_body) = gc_block(&formatted_guard.consequent) else {
        return false;
    };
    let [headers_loop] = formatted_body else {
        return false;
    };
    let Some((header_key, header_source, header_body)) = gc_for_in(headers_loop) else {
        return false;
    };
    if gc_symbol(model, header_source) != Some(formatted) {
        return false;
    }
    let Some(header_statements) = gc_block(header_body) else {
        return false;
    };
    let [last_key_write, header_copy_guard_statement] = header_statements else {
        return false;
    };
    let Some(last_key_assignment) = gc_plain_assignment(last_key_write) else {
        return false;
    };
    if assignment_symbol(model, last_key_assignment) != Some(last_key)
        || gc_symbol(model, &last_key_assignment.right) != Some(header_key)
    {
        return false;
    }
    let Statement::IfStatement(header_copy_guard) = header_copy_guard_statement else {
        return false;
    };
    if header_copy_guard.alternate.is_some() {
        return false;
    }
    let Some((copy_object, copy_key)) =
        gc_own_property_call(model, gc_strip_bangs(&header_copy_guard.test))
    else {
        return false;
    };
    if gc_symbol(model, copy_object) != Some(formatted)
        || gc_symbol(model, copy_key) != Some(last_key)
    {
        return false;
    }
    let Some(copy_statements) = gc_block(&header_copy_guard.consequent) else {
        return false;
    };
    let [copy_statement] = copy_statements else {
        return false;
    };
    let Some(copy_assignment) = gc_plain_assignment(copy_statement) else {
        return false;
    };
    gc_computed_target(model, &copy_assignment.left, dictionary, last_key)
        && gc_computed_read(model, &copy_assignment.right, formatted, last_key)
}
fn gc_header_key_binding(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statement: &Statement<'_>,
) -> Option<SymbolId> {
    let (pattern, init) = gc_single_declarator(statement)?;
    if init.is_some_and(|value| !descriptor_void_zero(value)) {
        return None;
    }
    let symbol = gc_declarator_symbol(pattern)?;
    declared_within_body(model, symbol, site).then_some(symbol)
}
fn gc_collection_init(
    model: &Model<'_, '_>,
    site: FunctionSite,
    all: &BTreeMap<u32, Facts<'_>>,
) -> Option<GcCollectionInit> {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return None;
    };
    if function.r#async || function.generator || function.params.rest.is_some() {
        return None;
    }
    if !function.params.items.is_empty() {
        return None;
    }
    let Some(body) = function.body.as_deref() else {
        return None;
    };
    if !body.directives.is_empty() {
        return None;
    }
    let [collector_save, collector_declaration, collector_install, optional] =
        body.statements.as_slice()
    else {
        return None;
    };
    let (target, saved_collector) = gc_save_named(model, collector_save, "ns_collect")?;
    if !gc_is_captured_symbol(model, site, target)
        || !gc_is_captured_symbol(model, site, saved_collector)
    {
        return None;
    }
    let (collector_symbol, collector_site) = gc_function_symbol(model, collector_declaration)?;
    if !declared_within_body(model, collector_symbol, site) {
        return None;
    }
    if !collect_encoded_headers(model, collector_site) {
        return None;
    }
    let (install_receiver, installed) = gc_install_named(model, collector_install, "ns_collect")?;
    if install_receiver != target || installed != collector_symbol {
        return None;
    }
    let Statement::IfStatement(optional) = optional else {
        return None;
    };
    let key = gc_symbol(model, &optional.test)?;
    if !gc_is_captured_symbol(model, site, key) {
        return None;
    }
    let Some(consequent) = gc_block(&optional.consequent) else {
        return None;
    };
    let [descriptor_save, mapper_declaration, descriptor_install, consequent_return] = consequent
    else {
        return None;
    };
    if !gc_returns_void(consequent_return) {
        return None;
    }
    let (save_receiver, save_key, saved_mapper) = gc_save_computed(model, descriptor_save)?;
    if save_receiver != target
        || save_key != key
        || !gc_is_captured_symbol(model, site, saved_mapper)
    {
        return None;
    }
    let (mapper_symbol, mapper_site) = gc_function_symbol(model, mapper_declaration)?;
    if !declared_within_body(model, mapper_symbol, site) {
        return None;
    }
    let mapper_facts = all.get(&mapper_site.span.start)?;
    if !beacon_descriptor_payload_record(model, mapper_site, mapper_facts, all) {
        return None;
    }
    let (install_receiver, install_key, installed_mapper) =
        gc_install_computed(model, descriptor_install)?;
    if install_receiver != target || install_key != key || installed_mapper != mapper_symbol {
        return None;
    }
    let Some(alternate_statement) = optional.alternate.as_ref() else {
        return None;
    };
    let Some(alternate) = gc_block(alternate_statement) else {
        return None;
    };
    let [alternate_return] = alternate else {
        return None;
    };
    if !gc_returns_void(alternate_return) {
        return None;
    }
    Some(GcCollectionInit {
        target,
        key,
        saved_collector,
        saved_mapper,
        collector: collector_site,
        mapper: mapper_site,
    })
}
fn initialise_global_collection_api(
    model: &Model<'_, '_>,
    site: FunctionSite,
    all: &BTreeMap<u32, Facts<'_>>,
) -> bool {
    gc_collection_init(model, site, all).is_some()
}
fn gc_alias_slot(
    model: &Model<'_, '_>,
    statement: &Statement<'_>,
    site: FunctionSite,
    slot: u32,
) -> bool {
    let Some((pattern, init)) = gc_single_declarator(statement) else {
        return false;
    };
    if gc_declarator_symbol(pattern).is_none() {
        return false;
    }
    let Some(init) = init else {
        return false;
    };
    model.site_parameter_slot(init, site) == Some(slot)
}
fn gc_uninitialized_pair(statement: &Statement<'_>) -> Option<(SymbolId, SymbolId)> {
    let Statement::VariableDeclaration(declaration) = statement else {
        return None;
    };
    let [first, second] = declaration.declarations.as_slice() else {
        return None;
    };
    if first.init.is_some() || second.init.is_some() {
        return None;
    }
    Some((
        gc_declarator_symbol(&first.id)?,
        gc_declarator_symbol(&second.id)?,
    ))
}
fn gc_target_global(
    model: &Model<'_, '_>,
    statement: &Statement<'_>,
    site: FunctionSite,
) -> Option<SymbolId> {
    let assignment = gc_plain_assignment(statement)?;
    let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left else {
        return None;
    };
    let target = crate::analysis::identifier_symbol(identifier, model.scoping)?;
    let Expression::StaticMemberExpression(member) = assignment.right.without_parentheses() else {
        return None;
    };
    if member.optional
        || member.property.name.as_str() != "global"
        || model.site_parameter_slot(&member.object, site) != Some(0)
    {
        return None;
    }
    Some(target)
}
fn gc_init_pair(
    model: &Model<'_, '_>,
    statement: &Statement<'_>,
    init_symbol: SymbolId,
) -> Option<SymbolId> {
    let (pattern, init) = gc_single_declarator(statement)?;
    let pair = gc_declarator_symbol(pattern)?;
    let Expression::ObjectExpression(object) = init?.without_parentheses() else {
        return None;
    };
    let [property] = object.properties.as_slice() else {
        return None;
    };
    let ObjectPropertyKind::ObjectProperty(property) = property else {
        return None;
    };
    if property.computed
        || property.method
        || property.kind != PropertyKind::Init
        || property_key_name(&property.key).as_deref() != Some("init")
        || gc_symbol(model, &property.value) != Some(init_symbol)
    {
        return None;
    }
    Some(pair)
}
fn gc_assign_named(
    model: &Model<'_, '_>,
    statement: &Statement<'_>,
    receiver: SymbolId,
    name: &str,
    value: SymbolId,
) -> bool {
    let Some(assignment) = gc_plain_assignment(statement) else {
        return false;
    };
    let AssignmentTarget::StaticMemberExpression(member) = &assignment.left else {
        return false;
    };
    !member.optional
        && member.property.name.as_str() == name
        && gc_symbol(model, &member.object) == Some(receiver)
        && gc_symbol(model, &assignment.right) == Some(value)
}
fn gc_global_collection_api(
    model: &Model<'_, '_>,
    site: FunctionSite,
    all: &BTreeMap<u32, Facts<'_>>,
) -> bool {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return false;
    };
    if function.r#async || function.generator || function.params.rest.is_some() {
        return false;
    }
    if function.params.items.len() != 4 {
        return false;
    }
    for parameter in &function.params.items {
        if parameter.initializer.is_some() {
            return false;
        }
        let BindingPattern::BindingIdentifier(identifier) = &parameter.pattern else {
            return false;
        };
        let Some(symbol) = identifier.symbol_id.get() else {
            return false;
        };
        if model.binding_writes(symbol) != 0
            || !model.scoping.symbol_redeclarations(symbol).is_empty()
        {
            return false;
        }
    }
    let Some(body) = function.body.as_deref() else {
        return false;
    };
    if !body.directives.is_empty() {
        return false;
    }
    let statements = body.statements.as_slice();
    if statements.len() != 11 {
        return false;
    }
    if !gc_alias_slot(model, &statements[0], site, 1)
        || !gc_alias_slot(model, &statements[2], site, 2)
    {
        return false;
    }
    let Some((first_slot, second_slot)) = gc_uninitialized_pair(&statements[1]) else {
        return false;
    };
    let Some(saved_collector) = gc_uninitialized_binding(model, site, &statements[3]) else {
        return false;
    };
    if !gc_alias_slot(model, &statements[4], site, 3) {
        return false;
    }
    let Some((teardown_symbol, teardown_site)) = gc_function_symbol(model, &statements[5]) else {
        return false;
    };
    let Some((init_symbol, init_site)) = gc_function_symbol(model, &statements[6]) else {
        return false;
    };
    if !declared_within_body(model, teardown_symbol, site)
        || !declared_within_body(model, init_symbol, site)
    {
        return false;
    }
    let Some(target) = gc_target_global(model, &statements[7], site) else {
        return false;
    };
    let Some(pair) = gc_init_pair(model, &statements[8], init_symbol) else {
        return false;
    };
    if !gc_assign_named(model, &statements[9], pair, "teardown", teardown_symbol)
        || gc_return_symbol(model, &statements[10]) != Some(pair)
    {
        return false;
    }
    let Some(init) = gc_collection_init(model, init_site, all) else {
        return false;
    };
    if init.target != target || init.saved_collector != saved_collector {
        return false;
    }
    if !((first_slot == target && second_slot == init.saved_mapper)
        || (second_slot == target && first_slot == init.saved_mapper))
    {
        return false;
    }
    gc_restore_pair(
        model,
        teardown_site,
        target,
        init.key,
        saved_collector,
        init.saved_mapper,
    )
}
fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    f: &Facts<'_>,
    all: &BTreeMap<u32, Facts<'_>>,
) -> Option<Proposal> {
    let make = |name, kind, detail| Some(proposal(name, kind, detail, f));
    // Exact live extension hook: message, constructor binding and single
    // statement are all evidence; arbitrary Error throwers do not qualify.
    if let AstKind::Function(function) = model.nodes.kind(site.node_id) {
        if let Some(body) = &function.body {
            if body.directives.is_empty() && body.statements.len() == 1 {
                if let Statement::ThrowStatement(thrown) = &body.statements[0] {
                    if let Expression::NewExpression(error) = &thrown.argument {
                        if expression_path(model, &error.callee) == ["Error"]
                            && error.arguments.len() == 1
                            && matches!(&error.arguments[0], Argument::StringLiteral(message)
                                if message.value == "serialisation is hard-coded into the encoder")
                        {
                            return make(
                                "throwHardcodedSerializationError",
                                "hardcoded_serialization_error",
                                "only statement throws unshadowed global Error with the exact hard-coded serialization message",
                            );
                        }
                    }
                }
            }
        }
    }
    if f.statements == 0 {
        return None;
    }
    if f.statements == 1 && f.returns.len() == 1 && f.calls.is_empty() {
        match f.returns[0] {
            Expression::BooleanLiteral(b) => {
                return make(
                    if b.value { "returnTrue" } else { "returnFalse" },
                    "constant_return",
                    "single literal boolean return",
                )
            }
            Expression::StringLiteral(s) if s.value.is_empty() => {
                return make(
                    "returnEmptyString",
                    "constant_return",
                    "single empty-string return",
                )
            }
            Expression::Identifier(i) => {
                if let Some(symbol) = crate::analysis::identifier_symbol(i, model.scoping) {
                    if model
                        .formal_symbol_slot(symbol)
                        .is_some_and(|slot| slot.owner == site.node_id)
                    {
                        return make(
                            "identity",
                            "formal_return",
                            "returns its unchanged formal parameter",
                        );
                    }
                    if model.binding_writes(symbol) == 0
                        && matches!(model.initializer(symbol),Some(Expression::ObjectExpression(o)) if o.properties.is_empty())
                    {
                        return make(
                            "getSharedEmptyObject",
                            "captured_object_return",
                            "returns one captured, initially empty object binding",
                        );
                    }
                }
            }
            _ => (),
        }
    }
    if f.statements == 1 && f.calls.len() == 1 && f.returns.is_empty() {
        let c = f.calls[0];
        let args = call_arguments(c);
        if static_member_name(&c.callee) == Some("error")
            && args.len() == 1
            && model.site_parameter_slot(args[0], site) == Some(0)
        {
            return make(
                "forwardError",
                "method_forwarder",
                "sole action forwards formal slot 0 to a captured receiver's error method",
            );
        }
    }
    if f.statements == 1 && f.returns.len() == 1 && f.calls.len() == 1 {
        let c = f.calls[0];
        if let Expression::StaticMemberExpression(m) = &c.callee {
            if m.property.name == "call" {
                if let Expression::Identifier(i) = &m.object {
                    if let Some(s) = crate::analysis::identifier_symbol(i, model.scoping) {
                        if model.binding_writes(s) == 0
                            || (model.binding_writes(s) == 1 && model.initializer(s).is_none())
                        {
                            if let Some(value) = model.symbol_value(s) {
                                let args = call_arguments(c);
                                if expression_path(model, value)
                                    == ["Object", "prototype", "hasOwnProperty"]
                                    && args.len() == 2
                                    && model.site_parameter_slot(args[0], site) == Some(0)
                                    && model.site_parameter_slot(args[1], site) == Some(1)
                                {
                                    return make("hasOwnProperty","resolved_api_alias","returns Object.prototype.hasOwnProperty.call through a scoped single-origin alias");
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    if global_call(model, f, &["Object", "assign", "apply"]) && f.loops > 0 && f.throws > 0 {
        return make(
            "assignObjectProperties",
            "object_assign_fallback",
            "calls global Object.assign.apply and retains an own-property copying fallback",
        );
    }
    if ((global_call(model, f, &["Object", "create"]) && has(f, &["prototype", "setPrototypeOf"]))
        || f.class_super_slots.contains(&0))
        && model.all_sites().iter().any(|child| {
            model
                .enclosing_function(child.node_id)
                .is_some_and(|parent| parent.span == site.span)
                && all.get(&child.span.start).is_some_and(|f| {
                    has(
                        f,
                        &[
                            "keysets",
                            "stringHist",
                            "remainingDepth",
                            "depthBoundExtensionPoint",
                        ],
                    ) && f.writes.contains("keysets")
                })
        })
    {
        return make("createValueEncoderClass","encoder_class_factory","constructs a prototype chain around a nested constructor that initializes the encoder state");
    }
    if f.globals.contains("Object")
        && has(f, &["setPrototypeOf"])
        && f.returns.len() == 1
        && f.calls.len() == 1
        && f.statements <= 3
        && f.assignments.iter().any(|assignment| {
            let AssignmentTarget::AssignmentTargetIdentifier(alias) = &assignment.left else {
                return false;
            };
            let Some(alias) = crate::analysis::identifier_symbol(alias, model.scoping) else {
                return false;
            };
            if expression_path(model, &assignment.right) != ["Object", "setPrototypeOf"] {
                return false;
            }
            let Expression::CallExpression(call) = f.returns[0] else {
                return false;
            };
            let args = call_arguments(call);
            crate::analysis::expression_symbol(&call.callee, model.scoping) == Some(alias)
                && args.len() == 2
                && model.site_parameter_slot(args[0], site) == Some(0)
                && model.site_parameter_slot(args[1], site) == Some(1)
        })
    {
        return make(
            "setObjectPrototype",
            "prototype_wrapper",
            "selects Object.setPrototypeOf or a fallback then returns the selected call",
        );
    }
    if f.writes.contains("__proto__")
        && f.returns.len() == 1
        && f.statements == 2
        && model.site_parameter_slot(f.returns[0], site) == Some(0)
    {
        if let AstKind::Function(function) = model.nodes.get_node(site.node_id).kind() {
            if let Some(body) = &function.body {
                if let Statement::ExpressionStatement(statement) = &body.statements[0] {
                    if let Expression::AssignmentExpression(a) = &statement.expression {
                        if let AssignmentTarget::StaticMemberExpression(m) = &a.left {
                            if m.property.name == "__proto__"
                                && model.site_parameter_slot(&m.object, site) == Some(0)
                                && model.site_parameter_slot(&a.right, site) == Some(1)
                            {
                                return make("setPrototypeFallback","prototype_assignment","assigns formal slot 1 to slot 0's __proto__ and returns slot 0");
                            }
                        }
                    }
                }
            }
        }
    }
    if f.globals.contains("Function")
        && has(f, &["prototype", "toString", "exec"])
        && f.regexes
            .iter()
            .any(|r| r.contains("native") && r.contains("code"))
    {
        return make(
            "isNativeFunction",
            "native_source_test",
            "Function.prototype.toString guard and native-code regular-expression test",
        );
    }
    if has(f, &["__callbacks", "indexOf"]) && f.writes.is_empty() && f.statements >= 2 {
        // Indexed writes are recorded through __callbacks reads; null removal
        // follows the indexOf result in this registry method.
        if f.assignments.iter().any(|a| {
            let AssignmentTarget::ComputedMemberExpression(clear) = &a.left else {
                return false;
            };
            if !this_callbacks(&clear.object) || !matches!(&a.right, Expression::NullLiteral(_)) {
                return false;
            }
            let Some(index) = crate::analysis::expression_symbol(&clear.expression, model.scoping)
            else {
                return false;
            };
            let Some(Expression::CallExpression(search)) = model.initializer(index) else {
                return false;
            };
            let args = call_arguments(search);
            args.len() == 3
                && expression_path(model, args[0]) == ["Array", "prototype", "indexOf"]
                && this_callbacks(args[1])
                && model.site_parameter_slot(args[2], site) == Some(0)
        }) {
            return make(
                "removeRegisteredCallback",
                "callback_index_removal",
                "uses Array.prototype.indexOf on this.__callbacks and clears the selected entry",
            );
        }
    }
    if has(f, &["__callbacks", "concat"])
        && f.assignments.iter().any(|a| {
            let AssignmentTarget::StaticMemberExpression(target) = &a.left else {
                return false;
            };
            if target.property.name != "__callbacks"
                || !matches!(&target.object, Expression::ThisExpression(_))
            {
                return false;
            }
            let Expression::CallExpression(call) = &a.right else {
                return false;
            };
            let args = call_arguments(call);
            args.len() == 3
                && expression_path(model, args[0]) == ["Array", "prototype", "concat"]
                && this_callbacks(args[1])
        })
    {
        return make(
            "mergeRegisteredCallbacks",
            "callback_concat",
            "replaces this.__callbacks using Array.prototype.concat",
        );
    }
    if global_call(model, f, &["Object", "defineProperty"])
        && has(
            f,
            &["enumerable", "configurable", "writable", "prototype", "key"],
        )
        && f.loops == 2
        && f.calls.len() == 2
        && f.returns.len() == 1
    {
        return make(
            "installPropertyDescriptors",
            "descriptor_loops",
            "normalizes descriptors in prototype/static loops and calls Object.defineProperty",
        );
    }
    if has(f, &["prototype", "test", "call"])
        && f.deletes > 0
        && f.returns.iter().any(|&e| {
            matches!(e, Expression::ObjectExpression(_))
                || placeholder_probe_record(model, site, f, e)
        })
    {
        return make(
            "inspectAndDeleteFunctionPrototype",
            "function_probe",
            "returns function-test fields and records a delete of the prototype property",
        );
    }
    if called(f, "createEvent") && called(f, "initCustomEvent") && f.returns.len() == 1
        && crate::analysis::expression_symbol(f.returns[0],model.scoping).is_some_and(|event|{
            let Some(Expression::CallExpression(create))=model.initializer(event) else{return false};
            let args=call_arguments(create);
            static_member_name(&create.callee)==Some("createEvent") && args.len()==1
                && matches!(args[0],Expression::StringLiteral(s) if s.value=="CustomEvent")
                && f.calls.iter().any(|c|matches!(&c.callee,Expression::StaticMemberExpression(m) if m.property.name=="initCustomEvent" && crate::analysis::expression_symbol(&m.object,model.scoping)==Some(event)))
        }) {
        return make(
            "createCustomEventFallback",
            "custom_event_factory",
            "creates an event, calls initCustomEvent and returns it",
        );
    }
    if called(f, "dispatchEvent") && f.loops > 0 {
        return make(if f.members.contains("detail") {"dispatchConfigCustomEvents"}else{"dispatchEmptyCustomEvents"},"custom_event_dispatch","loops over event names and dispatches constructed events; detail writes distinguish the configured variant");
    }
    if has(
        f,
        &[
            "keysets",
            "stringHist",
            "remainingDepth",
            "depthBoundExtensionPoint",
        ],
    ) && f.writes.contains("keysets")
    {
        return make(
            "initializeValueEncoder",
            "encoder_state",
            "initializes keysets, stringHist, remainingDepth and depthBoundExtensionPoint",
        );
    }
    if has(f, &["keysets", "stringHist", "STRLUT", "encodeValue"]) && called(f, "encodeValue") {
        return make(
            "encodeWithStringTable",
            "encoder_string_table",
            "encodes the value and keysets, then emits a STRLUT header and string/body bytes",
        );
    }
    if has(f, &["keysets", "push"]) && f.loops > 0 && f.returns.len() >= 1 {
        return make(
            "findOrAppendKeyset",
            "keyset_search",
            "searches keysets in a loop and returns an existing index or appends a keyset",
        );
    }
    if has(f, &["forEach"])
        && f.calls.len() == 1
        && f.loops == 0
        && f.returns.is_empty()
        && f.calls.iter().any(|c| {
            call_arguments(c).iter().any(|a| {
                model
                    .function_from_expression(a)
                    .and_then(|s| all.get(&s.span.start))
                    .is_some_and(|f| called(f, "encodeValue"))
            })
        })
    {
        return make(
            "encodeArrayElements",
            "encoder_element_loop",
            "forEach callback invokes encodeValue with the shared destination",
        );
    }
    if has(
        f,
        &[
            "extensions",
            "findKeysetIndex",
            "encodeValue",
            "remainingDepth",
            "EXTENSION",
        ],
    ) && called(f, "push")
    {
        return make(
            "encodeStructuredValue",
            "typed_value_encoder",
            "dispatches extension/keyset/value encodings and writes encoding tags and bytes",
        );
    }
    if f.news > 0 && has(f, &["depthBound", "extensions", "encode"]) && called(f, "encode") {
        return make(
            "encodeWithFreshEncoder",
            "fresh_encoder",
            "constructs an encoder using depthBound/extensions and returns its encode call",
        );
    }
    if f.returns.len() == 1 && f.calls.len() == 1 && local_call(model, f, "arrayContentsEqual") {
        let mut p = proposal(
            "matchesArrayContents",
            "lexical_call_wrapper",
            "returns the scoped arrayContentsEqual call",
            f,
        );
        p.confidence = Confidence::Medium;
        return Some(p);
    }
    if f.returns.len() == 1 && f.calls.len() == 1 && has(f, &["extensions", "detector"]) {
        return make(
            "testExtensionDetector",
            "extension_predicate",
            "returns an extension table entry's detector call",
        );
    }
    if called(f, "next")
        && called(f, "complete")
        && local_call(model, f, "numberFieldWriter")
        && local_call(model, f, "prngByteSourceFactory")
    {
        let mut p = proposal(
            "writeNumbersAndEmit",
            "lexical_writer_emission",
            "calls scoped numberFieldWriter and PRNG factory, then next and complete",
            f,
        );
        p.confidence = Confidence::Medium;
        return Some(p);
    }
    if called(f, "next")
        && global_call(model, f, &["Math", "random"])
        && f.returns
            .iter()
            .any(|r| matches!(r,Expression::StringLiteral(s) if s.value.is_empty()))
    {
        // A real getter installation (not a generated spelling or source comment)
        // supplies the stack-access context.
        if is_stack_getter(model, site) {
            return make("emitStackAccessProbe","stack_getter_emission","installed as Object.defineProperty(..., 'stack', {get: function}); emits seeded flag bytes and returns an empty string");
        }
    }
    if called(f, "map")
        && (f.calls.iter().any(|c| {
            call_arguments(c).iter().any(|a| {
                model
                    .function_from_expression(a)
                    .and_then(|s| all.get(&s.span.start))
                    .is_some_and(|f| {
                        f.writes.contains("beaconEndpoint") && f.writes.contains("beaconData")
                    })
            })
        }) || beacon_literal_endpoint_record(model, f, all))
    {
        return make(
            "buildBeaconReports",
            "beacon_record_map",
            "maps records to beaconEndpoint/beaconData objects",
        );
    }
    if beacon_descriptor_payload_record(model, site, f, all) {
        return make(
            "buildBeaconDescriptorPayloads",
            "beacon_descriptor_payload_map",
            "maps filtered records to beaconEndpoint/beaconDescriptor objects with one captured-encoder payload",
        );
    }
    if gc_global_collection_api(model, site, all)
        || runtime::collection_carrier_layout::matches(model, site, all)
    {
        return make(
            "createGlobalCollectionApi",
            "runtime_global_collection_api_carrier",
            "owned carrier: reads formal slot 0's global object, returns its own { init, teardown } pair whose owned initialiser installs an owned encoded-header collector on the protocol ns_collect slot and, on a captured dynamic slot, saves and installs an owned descriptor-payload mapper; the owned teardown restores exactly the two saved slots",
        );
    }
    if initialise_global_collection_api(model, site, all) {
        return make(
            "initialiseGlobalCollectionApi",
            "runtime_global_collection_api_initialiser",
            "zero-formal initialiser: saves the target ns_collect slot, installs this function's owned encoded-header collector, then on the captured dynamic key saves and installs an owned descriptor-payload mapper; every branch returns void",
        );
    }
    if collect_encoded_headers(model, site) {
        return make(
            "collectEncodedHeaders",
            "runtime_encoded_header_collector",
            "one request formal; a null/url/method/captured-policy guard; calls the captured payload generator with void 0, iterates its own keys, calls the captured header formatter with a prefixed loop key, the value and a numeric limit, then copies only the returned header's own keys into a null-prototype dictionary; returns the dictionary or null",
        );
    }
    None
}
fn this_callbacks(e: &Expression<'_>) -> bool {
    matches!(e,Expression::StaticMemberExpression(m) if m.property.name=="__callbacks" && matches!(&m.object,Expression::ThisExpression(_)))
}
fn is_stack_getter(model: &Model<'_, '_>, site: FunctionSite) -> bool {
    let Some(symbol) = site.binding_symbol else {
        return false;
    };
    model.scoping.get_resolved_references(symbol).any(|r| {
        let mut ancestors = model.nodes.ancestor_ids(r.node_id());
        let Some(property) = ancestors
            .find(|id| matches!(model.nodes.get_node(*id).kind(), AstKind::ObjectProperty(_)))
        else {
            return false;
        };
        let AstKind::ObjectProperty(p) = model.nodes.get_node(property).kind() else {
            return false;
        };
        if property_key_name(&p.key).as_deref() != Some("get") {
            return false;
        }
        model
            .nodes
            .ancestors(property)
            .find_map(|n| {
                if let AstKind::CallExpression(c) = n.kind() {
                    Some(c)
                } else {
                    None
                }
            })
            .is_some_and(|c| {
                let args = call_arguments(c);
                expression_path(model, &c.callee) == ["Object", "defineProperty"]
                    && args.len() == 3
                    && matches!(args[1],Expression::StringLiteral(s) if s.value=="stack")
            })
    })
}

pub(crate) fn proves_method_interceptor(
    model: &Model<'_, '_>, site: FunctionSite,
    function: &oxc_ast::ast::Function<'_>, body: &oxc_ast::ast::FunctionBody<'_>,
) -> bool {
    shared::proves_method_interceptor(model, site, function, body)
}
