//! Focused, source-preserving collector identity and routing. No naming planner,
//! wire grammar reconstruction, local naming or recovered JavaScript execution.
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use oxc_allocator::Allocator;
use oxc_span::GetSpan;
use serde::{Deserialize, Serialize};

use crate::signals::extract::bindings::Model;
pub use crate::signals::extract::scanner::grouped_registry::GroupedRegistration;
use crate::signals::extract::wiring::{collector_wiring, WiringError, WiringProperties};
use crate::signals::schema::{BindingRef, FunctionRef, ProgramPropertiesLimits, SourceIdentity, SourceSpan};
use crate::naming::program::{transform_program, ProgramBindingRename, ProgramTransformOptions};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ThreadIdentity {
    pub function_id: String,
    pub enclosing_function: Option<SourceSpan>,
    pub function: FunctionRef,
    pub original_thread_name: Option<String>,
    pub thread_id: Option<u64>,
    pub pc: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disambiguator: Option<u64>,
    pub identity_unresolved: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CollectorRename {
    pub function_id: String,
    pub binding: Option<BindingRef>,
    pub proposed_name: Option<String>,
    pub applied: bool,
    pub reason: Option<String>,
    pub reference_edits: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CollectorReport {
    pub schema_version: u32,
    pub source: SourceIdentity,
    pub transformed_source: SourceIdentity,
    /// All functions, including noncollectors, retain exact input coordinates.
    pub functions: Vec<ThreadIdentity>,
    /// `global_signal` in the reused wiring schema means descriptor index,
    /// never a final payload position. Rows also contain descriptor indices.
    pub wiring: WiringProperties,
    /// Identity proven by registry -> consumer -> six-argument invocation.
    /// This does not certify any inner emitter or signal route.
    #[serde(default)]
    pub registered_groups: Vec<GroupedRegistration>,
    /// Planned insertion edits for an anonymous producer's private identity
    /// holder. Insertions retain the original function and its lexical this.
    #[serde(default)]
    pub anonymous_naming_edits: Vec<crate::naming::program::ProgramSourceEdit>,
    pub payload_routing: super::collector_payload::PayloadRouting,
    pub renames: Vec<CollectorRename>,
    pub signal_routes: Vec<CollectorSignalRoute>,
    pub unresolved: Vec<String>,
    pub timings_ms: BTreeMap<String, f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CollectorSignalRoute {
    pub collector_function_id: Option<String>,
    pub wave_index: Option<usize>,
    pub row_slot: Option<usize>,
    pub descriptor_index: u32,
    /// Repeated descriptors in a filter can have multiple payload occurrences.
    pub default_payload_occurrences: Vec<super::collector_payload::PayloadSlot>,
    pub emitter_calls: Vec<crate::signals::extract::wiring::EmitterCallWiring>,
    pub unresolved: Vec<String>,
}

/// Union two independently proven positional paths. Wiring carries the existing
/// descriptor/row route proof; registry discovery can prove identity without
/// those rows. Corroborating span/slot facts are one claim, while different
/// positions remain separate claims and trigger the shared-factory refusal.
pub(crate) fn proven_group_positions<'a>(
    wiring: &'a WiringProperties,
    registered: &'a [GroupedRegistration],
) -> impl Iterator<Item = (&'a FunctionRef, usize)> {
    wiring.groups.iter().map(|g| (&g.function, g.group_index)).chain(
        registered.iter().filter(|g| g.unresolved.is_empty())
            .filter_map(|g| g.function.as_ref().map(|f| (f, g.slot))),
    )
}

/// Exact transformed bytes plus a report anchored to the ORIGINAL source.
#[derive(Debug)]
pub struct FocusedCollectors {
    pub source: String,
    pub report: CollectorReport,
}

/// Report all original function identities and structurally proven collectors.
/// `rename` opts into checked binding/reference edits and private identity
/// holders for capture-safe anonymous grouped producers. Shared factories are
/// reported and skipped; no semantic helper names are invented.
pub fn identify_collectors(source: &str, rename: bool) -> Result<FocusedCollectors, WiringError> {
    let total = Instant::now();
    let limits = ProgramPropertiesLimits::default();
    if source.trim().is_empty() {
        return Err(WiringError::InvalidInput("source is empty".into()));
    }
    if source.len() > limits.max_source_bytes {
        return Err(WiringError::Limit {
            what: "source bytes",
            limit: limits.max_source_bytes,
        });
    }
    let mut timings = BTreeMap::new();
    let allocator = Allocator::default();
    let parsed = crate::analysis::parse(&allocator, source);
    if parsed.panicked || !parsed.errors.is_empty() {
        return Err(WiringError::Parse(format!("{:?}", parsed.errors)));
    }
    let (scoping, nodes) = crate::analysis::analyze(&parsed.program);
    let model = Model::new(source, &parsed.program, &scoping, &nodes, limits)?;
    timings.insert(
        "parse_scope_index".into(),
        total.elapsed().as_secs_f64() * 1000.0,
    );
    let start = Instant::now();
    let functions = model.all_sites().iter().map(|site| {
        let function = model.function_ref(*site);
        let name = function.name_label.as_deref();
        let parsed = name.and_then(crate::naming::thread_id);
        ThreadIdentity {
            function_id: function_id(function.span),
            enclosing_function: model.enclosing_function(site.node_id).map(|s| s.span.into()),
            original_thread_name: name.filter(|n| n.starts_with("$THREAD_")).map(str::to_string),
            thread_id: parsed.map(|p| p.0), pc: parsed.map(|p| p.1), disambiguator: parsed.and_then(|p| p.2),
            identity_unresolved: parsed.is_none().then(||
                "no supported original $THREAD_<id>_<pc>[_<collision>] binding; unsupported prefixes remain unresolved and readable artifacts need original source".into()),
            function,
        }
    }).collect();
    let registered_groups = crate::signals::extract::scanner::grouped_registry::discover(&model);
    let dynamic_scope = crate::naming::program::has_dynamic_scope(&parsed.program);
    let anonymous_candidates: BTreeMap<_, _> = registered_groups.iter()
        .filter_map(|g| g.function.as_ref().filter(|f| f.binding.is_none()).map(|f| (g, f)))
        .map(|(group, function)| (function.span, anonymous_identity_edits(
            &model, function, &format!("waveCollector{}", group.slot + 1), dynamic_scope, &registered_groups,
        )))
        .collect();
    let properties = crate::signals::extract::scanner::collector_properties(&model, limits)?;
    let payload_routing =
        super::collector_payload::extract(&model, properties.descriptors.as_ref());
    timings.insert(
        "registry_rows_identity_payload".into(),
        start.elapsed().as_secs_f64() * 1000.0,
    );
    let start = Instant::now();
    let wiring = collector_wiring(model, properties, &mut timings)?;
    timings.insert(
        "collector_wiring".into(),
        start.elapsed().as_secs_f64() * 1000.0,
    );
    let mut signal_routes = Vec::new();
    let occurrences = |index: u32| {
        payload_routing
            .slots
            .iter()
            .filter(|s| s.descriptor_index == index as usize)
            .cloned()
            .collect()
    };
    for group in &wiring.groups {
        for (slot, index) in group.row.iter().copied().enumerate() {
            let emitter = group
                .local_emitters
                .iter()
                .find(|e| e.local_position == slot as u32);
            signal_routes.push(CollectorSignalRoute {
                collector_function_id: Some(function_id(group.function.span)),
                wave_index: Some(group.group_index),
                row_slot: Some(slot),
                descriptor_index: index,
                default_payload_occurrences: occurrences(index),
                emitter_calls: emitter.map_or_else(Vec::new, |e| e.calls.clone()),
                unresolved: if emitter.is_none() {
                    vec!["no resolved emitter sites for this row slot".into()]
                } else {
                    Vec::new()
                },
            });
        }
    }
    for route in &wiring.direct_routes {
        signal_routes.push(CollectorSignalRoute {
            collector_function_id: route.ctor.callback.as_ref().map(|f| function_id(f.span)),
            wave_index: None,
            row_slot: None,
            descriptor_index: route.descriptor_index,
            default_payload_occurrences: occurrences(route.descriptor_index),
            emitter_calls: route
                .local_emitters
                .iter()
                .flat_map(|e| e.calls.clone())
                .collect(),
            unresolved: route.unresolved.clone(),
        });
    }
    let mut candidates: BTreeMap<SourceSpan, (FunctionRef, Vec<String>)> = BTreeMap::new();
    let mut unresolved = Vec::new();
    for group in &registered_groups {
        if !group.unresolved.is_empty() {
            unresolved.push(format!(
                "registry {} slot {}: {}",
                group.registry.name,
                group.slot,
                group.unresolved.join("; ")
            ));
            continue;
        }
    }
    let mut grouped_spans = BTreeSet::new();
    for (function, slot) in proven_group_positions(&wiring, &registered_groups) {
        grouped_spans.insert(function.span);
        let entry = candidates
            .entry(function.span)
            .or_insert_with(|| (function.clone(), Vec::new()));
        let name = format!("waveCollector{}", slot + 1);
        if !entry.1.contains(&name) {
            entry.1.push(name);
        }
    }
    for route in &wiring.direct_routes {
        // The wrapper is not the collector. A missing producer is a refusal.
        if let Some(function) = &route.ctor.callback {
            let entry = candidates
                .entry(function.span)
                .or_insert_with(|| (function.clone(), Vec::new()));
            entry.1.push(format!(
                "directCollectorDescriptor{}",
                route.descriptor_index
            ));
        } else {
            unresolved.push(format!(
                "descriptor {}: no proven direct callback binding",
                route.descriptor_index
            ));
        }
    }
    // Different function bodies sharing a lexical binding cannot be renamed
    // independently. Also refuse mutable/reassigned bindings below.
    let mut binding_bodies: BTreeMap<u32, BTreeSet<SourceSpan>> = BTreeMap::new();
    for (span, (function, _)) in &candidates {
        if let Some(binding) = &function.binding {
            binding_bodies
                .entry(binding.identifier.start)
                .or_default()
                .insert(*span);
        }
    }
    let mut renames = Vec::new();
    let mut plans = Vec::new();
    let mut anonymous_naming_edits = Vec::new();
    for (span, (function, names)) in candidates {
        let binding = function.binding.clone();
        let reason = if names.len() != 1 {
            Some("shared factory serves multiple routes; no unique collector name".into())
        } else if let Some(binding) = &binding {
            let grouped = grouped_spans.contains(&span);
            if thread_identity(&binding.name).is_none() && !grouped {
                renames.push(CollectorRename {
                    function_id: function_id(span),
                    binding: Some(binding.clone()),
                    proposed_name: Some(names[0].clone()),
                    applied: false,
                    reason: Some("existing readable collector binding retained".into()),
                    reference_edits: 0,
                });
                continue;
            }
            let symbol = scoping
                .symbol_ids()
                .find(|s| s.index() as u32 == binding.symbol);
            if binding_bodies[&binding.identifier.start].len() != 1
                || symbol.is_none_or(|s| scoping.get_resolved_references(s).any(|r| r.is_write()))
            {
                Some("collector binding has multiple bodies or assignment writes".into())
            } else {
                None
            }
        } else {
            let grouped = grouped_spans.contains(&span);
            if grouped {
                match anonymous_candidates.get(&span).cloned().unwrap_or_else(|| Err("anonymous registry identity has no safe plan".into())) {
                    Ok(edits) => {
                        anonymous_naming_edits.extend(edits);
                        None
                    }
                    Err(reason) => Some(reason),
                }
            } else {
                Some("anonymous/method collector has no editable binding".into())
            }
        };
        let proposed_name = (names.len() == 1).then(|| names[0].clone());
        if reason.is_none() && rename && binding.is_some() {
            plans.push(ProgramBindingRename::new(
                binding.as_ref().unwrap().identifier.start,
                names[0].clone(),
            ));
        }
        renames.push(CollectorRename {
            function_id: function_id(span),
            binding,
            proposed_name,
            applied: false,
            reason: reason.or_else(|| (!rename).then(|| "report only; rename disabled".into())),
            reference_edits: 0,
        });
    }
    let start = Instant::now();
    let mut binding_edits = Vec::new();
    let mut output = if rename && !plans.is_empty() {
        let result = transform_program(source, &plans, &[], &ProgramTransformOptions::default())
            .map_err(|e| WiringError::InvalidInput(e.to_string()))?;
        for outcome in &result.renames {
            if let Some(entry) = renames.iter_mut().find(|r| {
                r.binding
                    .as_ref()
                    .is_some_and(|b| b.identifier.start == outcome.declaration_start)
            }) {
                entry.applied = outcome.applied;
                entry.reason = outcome.skip_reason.as_ref().map(|r| format!("{r:?}"));
                entry.reference_edits = outcome.reference_edits;
            }
        }
        binding_edits = result.edits;
        result.transformed_source
    } else {
        source.to_string()
    };
    if rename && !anonymous_naming_edits.is_empty() {
        binding_edits.extend(anonymous_naming_edits.clone());
        output = checked_identity_edits(source, &parsed.program, &binding_edits)
            .map_err(WiringError::InvalidInput)?;
        for entry in &mut renames {
            if entry.binding.is_none() && entry.reason.is_none() {
                entry.applied = true;
            }
        }
    }
    timings.insert(
        "checked_binding_rename".into(),
        start.elapsed().as_secs_f64() * 1000.0,
    );
    let transformed_source = SourceIdentity {
        sha256: crate::source::sha256_hex(output.as_bytes()),
        bytes: output.len(),
    };
    timings.insert("total".into(), total.elapsed().as_secs_f64() * 1000.0);
    Ok(FocusedCollectors {
        source: output,
        report: CollectorReport {
            schema_version: 1,
            source: wiring.source.clone(),
            transformed_source,
            functions,
            wiring,
            registered_groups,
            anonymous_naming_edits,
            payload_routing,
            renames,
            signal_routes,
            unresolved,
            timings_ms: timings,
        },
    })
}

fn function_id(span: SourceSpan) -> String {
    format!("fn@{}..{}", span.start, span.end)
}

fn thread_identity(name: &str) -> Option<(u64, u64)> {
    crate::naming::thread_id(name).map(|(id, pc, _)| (id, pc))
}

fn anonymous_identity_edits(
    model: &Model<'_, '_>,
    function: &FunctionRef,
    name: &str,
    dynamic_scope: bool,
    groups: &[GroupedRegistration],
) -> Result<Vec<crate::naming::program::ProgramSourceEdit>, String> {
    use oxc_ast::AstKind;
    let span = function.span;
    let site = model
        .site_by_span(oxc_span::Span::new(span.start, span.end))
        .ok_or("anonymous registry function is not indexed")?;
    if dynamic_scope
        || !matches!(
            model.nodes.kind(site.node_id),
            AstKind::ArrowFunctionExpression(_) | AstKind::Function(_)
        )
    {
        return Err("anonymous registry identity holder requires an ordinary function expression and no dynamic scope".into());
    }
    // The wrapper arrow introduces exactly one private const. No existing read
    // (including free, captured, and nested reads) may acquire that binding.
    // Include parameter initializers as well as the body.
    let inside = |node| {
        let reference = model.nodes.get_node(node).span();
        span.start <= reference.start && reference.end <= span.end
    };
    if model.nodes.iter().any(|node| {
        inside(node.id())
            && matches!(node.kind(), AstKind::IdentifierReference(id) if id.name == name)
    }) {
        return Err("anonymous registry identity name would capture an existing reference".into());
    }
    // Also account for simultaneous canonical renames that could introduce a
    // captured reference with this spelling after the original-source check.
    for group in groups
        .iter()
        .filter(|g| format!("waveCollector{}", g.slot + 1) == name)
    {
        if let Some(binding) = group.function.as_ref().and_then(|f| f.binding.as_ref()) {
            let Some(symbol) = model.symbol_by_index(binding.symbol) else {
                continue;
            };
            if model
                .scoping
                .get_resolved_references(symbol)
                .any(|r| inside(r.node_id()))
            {
                return Err("anonymous registry identity name would capture a simultaneous positional rename".into());
            }
        }
    }
    Ok(vec![
        crate::naming::program::ProgramSourceEdit {
            start: span.start,
            end: span.start,
            replacement: format!("(()=>{{const {name} = "),
        },
        crate::naming::program::ProgramSourceEdit {
            start: span.end,
            end: span.end,
            replacement: format!(";return {name};}})()"),
        },
    ])
}

pub(crate) fn checked_identity_edits(
    source: &str,
    program: &oxc_ast::ast::Program<'_>,
    edits: &[crate::naming::program::ProgramSourceEdit],
) -> Result<String, String> {
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
        return Err("overlapping registry identity edits".into());
    }
    crate::naming::program::validate_rewrite(program, &applied.output)
        .map_err(|e| format!("registry identity validation: {e:?}"))?;
    Ok(applied.output)
}
