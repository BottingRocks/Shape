//! `MemoryBreakdownSummary`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::extract::contracts::SourceClass;
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, LayoutNode, LeafKind, LeafOp, LeafView, SignalLayout, SlotView};
use crate::signals::analysis::{leaf_checks::*, field_solver::*};
use crate::signals::catalog::card::{Card, CardBody, CardField, CardMessage};


/// `MemoryBreakdownSummary`. Card: [`CARD`].
pub(crate) fn recognize_memory_breakdown_summary(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    let leaves = leaves_of(view);
    let result = any_transform(&leaves, ".breakdown") && any_transform(&leaves, ".bytes");
    let status_only = view.emissions().iter().any(|emission| {
        emission.leaves.len() == 4
            && emission.leaves.iter().all(literal_only)
            && emission.leaves.iter().filter(|leaf| leaf.op == LeafOp::Number).count() == 1
    });
    if result && status_only {
        return claim(&["status-only message (status + 3 markers)", "result.breakdown / result.bytes"]);
    }
    None
}

pub(crate) fn bind_memory_breakdown_summary(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    // Two messages: status-only (status + three literal placeholders) and the
    // result. Lists are length-prefixed: a count sits right before its
    // repeat. Attribution strings read the collector's own record keys; each
    // key is resolved to the value stored under it where the attribution
    // record is built (url / scope / container.src / container.id).
    let mut binding = FieldBinding::new();
    let all_messages: Vec<Vec<Field>> = (0..layout.messages.len())
        .map(|message| message_fields(view, layout, message))
        .collect::<Result<_, _>>()?;
    // A placeholder byte computed through a re-assigned mask register
    // (`m = m(); out[i] = m ^ 132`) reaches the contract as an opaque value;
    // it still has no host or loop source.
    let marker_leaf = |leaf: &LeafView| {
        !leaf.classes.is_empty()
            && leaf.classes.iter().all(|class| matches!(class, SourceClass::Literal | SourceClass::Opaque))
    };
    let status_only = |fields: &[Field]| {
        fields.iter().all(|field| field.literal_only())
            || fields.iter().all(|field| {
                (field.is_plain(LeafKind::Byte) || field.is_plain(LeafKind::Number))
                    && !field.leaves.is_empty()
                    && field.leaves.iter().all(marker_leaf)
            }) && fields.iter().any(|field| field.literal_only() && field.is_plain(LeafKind::Number))
    };
    let mut result_slots: Option<Vec<&'static str>> = None;
    for (message, fields) in all_messages.iter().enumerate().filter(|(_, fields)| !status_only(fields)) {
        let members: Vec<Vec<String>> = fields
            .iter()
            .map(|field| field_slice(ctx, field, 2).nearest_members().into_iter().map(str::to_string).collect())
            .collect();
        let reads = |field: &Field, name: &str| {
            fields
                .iter()
                .position(|other| other.key == field.key)
                .is_some_and(|index| members[index].iter().any(|member| member == name))
        };
        let before_array = |field: &Field| {
            fields
                .iter()
                .position(|other| other.key == field.key)
                .and_then(|index| fields.get(index + 1))
                .is_some_and(|next| next.is_array())
        };
        let status = |f: &Field| f.is_plain(LeafKind::Number) && f.literal_only();
        let entry_count = |f: &Field| f.is_plain(LeafKind::Number) && !f.literal_only() && before_array(f);
        let entry = |f: &Field| f.item_fields().is_some();
        let total_bytes = |f: &Field| {
            f.is_plain(LeafKind::Number) && !before_array(f) && reads(f, "bytes") && !reads(f, "length")
        };
        let breakdown_count = |f: &Field| {
            f.is_plain(LeafKind::Number) && !before_array(f) && reads(f, "breakdown") && reads(f, "length")
        };
        let mut local = FieldBinding::new();
        solve(
            message,
            fields,
            &[
                ("status", &status),
                ("entryCount", &entry_count),
                ("entry", &entry),
                ("totalBytes", &total_bytes),
                ("breakdownCount", &breakdown_count),
            ],
            &mut local,
        )?;
        // Top-level data slots, `entryCount` + `entry` as one.
        let mut slots = Vec::new();
        for field in fields {
            match local.get(message, &field.key) {
                Some("entryCount") => {}
                Some("entry") => slots.push("entries"),
                Some("status") => slots.push("status"),
                Some("totalBytes") => slots.push("totalBytes"),
                Some("breakdownCount") => slots.push("breakdownCount"),
                _ => return Err(BindRefusal::new("MemoryBreakdownSummary: unnamed result field")),
            }
            if let Some(name) = local.get(message, &field.key) {
                binding.insert(message, field.key.clone(), name);
            }
        }
        if result_slots.replace(slots).is_some() {
            return Err(BindRefusal::new("MemoryBreakdownSummary: two result messages"));
        }
        for field in fields.iter().filter(|field| field.is_array()) {
            let entry = field
                .item_fields()
                .ok_or_else(|| BindRefusal::new("MemoryBreakdownSummary: entry is not a record"))?;
            bind_breakdown_entry(ctx, message, &entry, &mut binding)?;
        }
    }
    let slots = result_slots.ok_or_else(|| BindRefusal::new("MemoryBreakdownSummary: no result message"))?;
    for (message, fields) in all_messages.iter().enumerate().filter(|(_, fields)| status_only(fields)) {
        // The placeholders stand at the positions of the result's data slots.
        if fields.len() != slots.len() {
            return Err(BindRefusal::new(format!(
                "MemoryBreakdownSummary: message {message}: {} fields but the result has {} slots",
                fields.len(),
                slots.len()
            )));
        }
        for (field, slot) in fields.iter().zip(&slots) {
            let name = match (*slot, field.is_plain(LeafKind::Number), field.is_plain(LeafKind::Byte)) {
                ("status", true, _) => "status",
                ("entries", _, true) => "entriesPlaceholder",
                ("totalBytes", _, true) => "totalBytesPlaceholder",
                ("breakdownCount", _, true) => "breakdownCountPlaceholder",
                _ => {
                    return Err(BindRefusal::new(format!(
                        "MemoryBreakdownSummary: message {message}: field {} does not line up with the result's {slot}",
                        field.key
                    )))
                }
            };
            binding.insert(message, field.key.clone(), name);
        }
    }
    Ok(binding)
}

/// One breakdown entry record: `typeCount` + `memoryType` repeat,
/// `attributionCount` + `attribution` repeat, `entryBytes`.
fn bind_breakdown_entry(
    ctx: &FinderCtx,
    message: usize,
    fields: &[Field],
    binding: &mut FieldBinding,
) -> Result<(), BindRefusal> {
    let next = |field: &Field| {
        fields
            .iter()
            .position(|other| other.key == field.key)
            .and_then(|index| fields.get(index + 1))
    };
    let string_array = |f: &Field| matches!(f.node, LayoutNode::Array { item } if matches!(item.as_ref(), LayoutNode::Leaf(leaf) if leaf.kind == LeafKind::String));
    let count_of = |f: &Field, list: &dyn Fn(&Field) -> bool| f.is_plain(LeafKind::Number) && next(f).is_some_and(|n| list(n));
    let record_array = |f: &Field| f.item_fields().is_some();
    let type_count = |f: &Field| count_of(f, &string_array);
    let attribution_count = |f: &Field| count_of(f, &record_array);
    let entry_bytes = |f: &Field| f.is_plain(LeafKind::Number) && !next(f).is_some_and(|n| n.is_array());
    solve(
        message,
        fields,
        &[
            ("typeCount", &type_count),
            ("memoryType", &string_array),
            ("attributionCount", &attribution_count),
            ("attribution", &record_array),
            ("entryBytes", &entry_bytes),
        ],
        binding,
    )?;
    for field in fields.iter().filter(|field| record_array(field)) {
        let attribution = field.item_fields().unwrap_or_default();
        bind_attribution(ctx, message, &attribution, binding)?;
    }
    Ok(())
}

/// One attribution record: `containerMarker` (no container) and four
/// strings named by the value their record key stores.
fn bind_attribution(
    ctx: &FinderCtx,
    message: usize,
    fields: &[Field],
    binding: &mut FieldBinding,
) -> Result<(), BindRefusal> {
    // The record key path each string field reads at its write site.
    let keys: Vec<Option<Vec<String>>> = fields
        .iter()
        .map(|field| {
            let mut paths: Vec<Vec<String>> = field
                .leaves
                .iter()
                .filter(|leaf| leaf.op == LeafOp::String)
                .filter_map(|leaf| ctx.caller_value_node(leaf).and_then(|node| ctx.read_key_path(node)))
                .collect();
            paths.dedup();
            match paths.as_slice() {
                [path] if !path.is_empty() => Some(path.clone()),
                _ => None,
            }
        })
        .collect();
    let mut first: Vec<&str> = keys.iter().flatten().map(|path| path[0].as_str()).collect();
    first.sort_unstable();
    first.dedup();
    // The constructions of the attribution record: every path resolves
    // through it (nested keys through the object literal stored under the
    // outer key) to a stored value.
    let classify = |value| {
        let slice = ctx.slice_of(value, 1);
        let members = slice.nearest_members();
        let has = |name: &str| members.contains(&name);
        match (has("container"), has("src"), has("id"), has("scope"), has("url")) {
            (true, true, false, false, false) => Some("containerSrc"),
            (true, false, true, false, false) => Some("containerId"),
            (false, false, false, true, false) => Some("scope"),
            (false, false, false, false, true) => Some("url"),
            _ => None,
        }
    };
    // A nested record stored directly (`o.j = {…}`) or through a local
    // holding the literal (`c = {…}; o.j = c`): its one definition.
    let record_of = |value: oxc_semantic::NodeId| {
        ctx.record_at(value).or_else(|| {
            let slice = ctx.slice_of(value, 1);
            match slice.definitions.as_slice() {
                [(_, definition, _)] => ctx.definition_value(*definition).and_then(|literal| ctx.record_at(literal)),
                _ => None,
            }
        })
    };
    let walk = |start: &crate::signals::analysis::write_sites::RecordConstruction, path: &[String]| {
        let mut record = start.clone();
        let mut value = *record.values.get(&path[0])?;
        for key in &path[1..] {
            record = record_of(value)?;
            value = *record.values.get(key)?;
        }
        Some(value)
    };
    // A read whose record binding is re-pointed at a nested record on one
    // arm (`c = c.j` / `c = other`) keeps only its last key: it resolves
    // through the one nested record (stored under a key of the root) that
    // holds that key.
    let resolve = |root: &crate::signals::analysis::write_sites::RecordConstruction, path: &[String]| {
        if root.values.contains_key(&path[0]) {
            return walk(root, path);
        }
        let mut nested = root
            .values
            .values()
            .filter_map(|value| record_of(*value))
            .filter(|record| record.values.contains_key(&path[0]));
        let inner = nested.next()?;
        if nested.next().is_some() {
            return None;
        }
        walk(&inner, path)
    };
    // Whether every read key path of `root` resolves (by `step`) to a value
    // that classifies.
    let classifies = |root: &crate::signals::analysis::write_sites::RecordConstruction,
                      step: &dyn Fn(&crate::signals::analysis::write_sites::RecordConstruction, &[String]) -> Option<oxc_semantic::NodeId>| {
        keys.iter().flatten().all(|path| step(root, path).and_then(classify).is_some())
    };
    let mut candidates = ctx.records_with_keys(&first);
    if !candidates.iter().any(|root| classifies(root, &walk)) {
        // No construction stores every key directly: the roots that resolve
        // every key, nested ones through their one nested record.
        let mut seen = std::collections::HashSet::new();
        candidates = first
            .iter()
            .flat_map(|key| ctx.records_with_keys(&[key]))
            .filter(|record| seen.insert(record.node))
            .filter(|record| classifies(record, &resolve))
            .collect();
    }
    let mut classes: Vec<Option<&'static str>> = vec![None; fields.len()];
    let mut roots = 0;
    for root in candidates {
        let resolved: Option<Vec<Option<&'static str>>> = keys
            .iter()
            .map(|path| match path {
                None => Some(None),
                Some(path) => resolve(&root, path).map(classify),
            })
            .collect();
        let Some(resolved) = resolved else {
            continue;
        };
        roots += 1;
        if roots == 1 {
            classes = resolved;
        } else if classes != resolved {
            return Err(BindRefusal::new(
                "MemoryBreakdownSummary: attribution keys resolve to record constructions that disagree",
            ));
        }
    }
    let stored = classes;
    let class_of = |field: &Field| {
        fields
            .iter()
            .position(|other| other.key == field.key)
            .and_then(|index| stored[index])
    };
    let marker = |f: &Field| matches!(f.node, LayoutNode::Choice { .. }) && f.literal_only();
    let named = |name: &'static str| move |f: &Field| f.writes(LeafKind::String) && class_of(f) == Some(name);
    let (src, id, scope, url) = (named("containerSrc"), named("containerId"), named("scope"), named("url"));
    solve(
        message,
        fields,
        &[
            ("containerMarker", &marker),
            ("containerSrc", &src),
            ("containerId", &id),
            ("scope", &scope),
            ("url", &url),
        ],
        binding,
    )
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `MemoryBreakdownSummary`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "MemoryBreakdownSummary",
    summary: "Uses performance.measureUserAgentSpecificMemory() to ask the browser how much memory the page uses. It first reports a status (started, API missing, request pending). When the measurement arrives it sends up to five memory-breakdown entries that belong to this page or its windows/workers (the current page's own entry first), each with its attributions (url, scope, iframe container) and memory types, plus the total bytes and the total number of breakdown entries. Only the newest message counts.",
    why: "Whether this API exists (it needs a cross-origin-isolated Chromium page) and what the memory numbers look like likely helps confirm the real browser and environment; instrumented or headless browsers, extra injected frames or workers may show up in the breakdown.",
    sample_payload: Some(r#"{
  "breakdownCountPlaceholder": null,
  "entriesPlaceholder": null,
  "status": 3,
  "totalBytesPlaceholder": null
}"#),
    by_name: false,
    body: CardBody::Messages(&[
        CardMessage {
            when: "Status only: right at setup (status 0), when the API is missing (status 3), or when the request has been sent and is pending (status 1)",
            fields: &[
                CardField::number(
                    "status",
                    "0 = set up, collection not run yet; 1 = measurement requested, result pending (or it failed); 3 = measureUserAgentSpecificMemory not available.",
                ),
                CardField::null(
                    "entriesPlaceholder",
                    "Null when no measurement entry list is available; the concrete marker byte is build-specific.",
                ),
                CardField::null(
                    "totalBytesPlaceholder",
                    "Null when no measurement total is available; the concrete marker byte is build-specific.",
                ),
                CardField::null(
                    "breakdownCountPlaceholder",
                    "Null when no breakdown count is available; the concrete marker byte is build-specific.",
                ),
            ],
        },
        CardMessage {
            when: "Measurement result arrived (status 2)",
            fields: &[
                CardField::number("status", "Always 2 = measurement done."),
                CardField::number("entryCount", "Number of breakdown entries that follow, 0 to 5."),
                CardField::group("entry", &[
                    CardField::number("attributionCount", "Number of attributions in this entry."),
                    CardField::group("attribution", &[
                        CardField::null(
                            "containerMarker",
                            "Null marker byte written instead of containerSrc and containerId when the attribution has no container element.",
                        ),
                        CardField::string(
                            "containerSrc",
                            "src of the iframe/frame element that holds this context (written only when a container exists; otherwise containerMarker is written instead of containerSrc and containerId).",
                        ),
                        CardField::string(
                            "containerId",
                            "id of that container element (written only when a container exists).",
                        ),
                        CardField::string(
                            "scope",
                            "Kind of context, e.g. \"Window\", \"DedicatedWorkerGlobalScope\", \"SharedWorkerGlobalScope\".",
                        ),
                        CardField::string(
                            "url",
                            "URL of the context with http:// or https:// removed, first 99 characters.",
                        ),
                    ])
                    .repeat("once per attribution, attributionCount times"),
                    CardField::number("typeCount", "Number of memory type labels for this entry."),
                    CardField::string("memoryType", "Memory type label, e.g. \"JavaScript\" or \"DOM\".")
                        .repeat("once per type label, typeCount times"),
                    CardField::number("entryBytes", "Bytes of memory attributed to this entry."),
                ])
                .repeat("once per reported breakdown entry, entryCount times (last entry first)"),
                CardField::number("totalBytes", "Total bytes reported by the measurement (result.bytes)."),
                CardField::number(
                    "breakdownCount",
                    "Total number of entries in the full breakdown, before filtering and the cap of 5.",
                ),
            ],
        },
    ]),
};
