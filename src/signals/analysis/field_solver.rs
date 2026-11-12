//! Naming fields by forced uniqueness: each projected top-level field is
//! paired with the view leaves it covers ([`Field`]), every card name is a
//! rule over the field (leaf kinds, provenance, or its write-site
//! [`ValueSlice`]), and [`solve`] assigns a name only when exactly one free
//! rule matches, refusing as soon as a field has none or several remain.
//!
//! For binders whose field order is shuffled per build and whose leaf
//! provenance can carry extra sources through shared registers.
//!
//! Used by: `DocumentReferrerHistory`, `DomElementStatistics`, `FeaturePolicy`,
//! `HostEnvironment`, `IncognitoStorageProbe`, `IntlLocaleTimeZone`,
//! `LocationUrlComponents`, `MatchMedia`, `MemoryBreakdownSummary`,
//! `NavigatorPlugins`, `NavigatorProperties`, `OuterArgsGlobalProcess`,
//! `ShuffledDateString`, `UserAgentData`, `WebRtcIceCandidateAddresses`,
//! `WorkerRealmProfile`.

use crate::signals::extract::contracts::SourceClass;
use super::layout_nodes::{kind_matches, node_kinds, node_leaf_count, writes_value};
use super::leaf_checks::{has_api, has_class, has_transform, literal_only};
use super::{BindRefusal, FieldBinding, FinderCtx, LayoutField, LayoutNode, LeafKind, LeafView, SignalLayout, SlotView, ValueSlice};

/// One projected field together with the value leaves it covers.
pub(crate) struct Field<'a> {
    pub(crate) key: String,
    pub(crate) node: &'a LayoutNode,
    pub(crate) leaves: &'a [LeafView],
}

/// Pair each field with its run of `leaves` (by leaf count).
pub(crate) fn split_fields<'a>(fields: &'a [LayoutField], leaves: &'a [LeafView]) -> Vec<Field<'a>> {
    let mut out = Vec::new();
    let mut at = 0usize;
    for field in fields {
        let count = node_leaf_count(&field.node);
        let end = (at + count).min(leaves.len());
        out.push(Field {
            key: field.key.clone(),
            node: &field.node,
            leaves: &leaves[at.min(end)..end],
        });
        at += count;
    }
    out
}

/// The top-level fields of message `message` with the leaves of the first
/// emission (part) that projects to that message.
pub(crate) fn message_fields<'a>(
    view: &'a SlotView,
    layout: &'a SignalLayout,
    message: usize,
) -> Result<Vec<Field<'a>>, BindRefusal> {
    let target = layout
        .messages
        .get(message)
        .ok_or_else(|| BindRefusal::new(format!("no message {message}")))?;
    for emission in &layout.emissions {
        let Some(emission_view) = view.emissions().get(emission.emission) else {
            continue;
        };
        let leaves = emission_view.leaves.as_slice();
        for (index, (_, part)) in emission.parts.iter().enumerate() {
            if part.mask == target.mask && part.fields == target.fields {
                if !emission.covers(leaves.len()) {
                    return Err(BindRefusal::new(format!(
                        "message {message}: layout parts do not cover the emission's {} leaves",
                        leaves.len()
                    )));
                }
                // Fields borrow a run of leaves: a part must cover one.
                let indices = emission.part_leaves(index);
                let (first, count) = (indices.first().copied().unwrap_or(0), indices.len());
                if indices.iter().enumerate().any(|(at, leaf)| *leaf != first + at) {
                    return Err(BindRefusal::new(format!(
                        "message {message}: the part's leaves are not one run of the emission"
                    )));
                }
                let part_leaves = &leaves[first..first + count];
                let mut kinds = Vec::new();
                part.fields.iter().for_each(|field| node_kinds(&field.node, &mut kinds));
                if kinds.len() != part_leaves.len()
                    || kinds.iter().zip(part_leaves).any(|(kind, leaf)| !kind_matches(*kind, leaf.op))
                {
                    return Err(BindRefusal::new(format!(
                        "message {message}: layout leaf kinds differ from the emission leaves"
                    )));
                }
                return Ok(split_fields(&part.fields, part_leaves));
            }
        }
    }
    Err(BindRefusal::new(format!("message {message}: no emission projects to it")))
}

impl<'a> Field<'a> {
    /// Some leaf has provenance transform `transform`.
    pub(crate) fn tr(&self, transform: &str) -> bool {
        self.leaves.iter().any(|leaf| has_transform(leaf, transform))
    }

    /// Some leaf reads an API path matching `suffix`.
    pub(crate) fn api(&self, suffix: &str) -> bool {
        self.leaves.iter().any(|leaf| has_api(leaf, suffix))
    }

    /// Some leaf has source class `class`.
    pub(crate) fn class(&self, class: SourceClass) -> bool {
        self.leaves.iter().any(|leaf| has_class(leaf, class))
    }

    /// The field writes a non-marker value of `kind` ([`writes_value`]).
    pub(crate) fn writes(&self, kind: LeafKind) -> bool {
        writes_value(self.node, kind)
    }

    /// A single plain (non-choice, non-array) leaf of `kind`.
    pub(crate) fn is_plain(&self, kind: LeafKind) -> bool {
        match self.node {
            LayoutNode::Leaf(leaf) => leaf.kind == kind,
            LayoutNode::ByteSelect { .. } => kind == LeafKind::Byte,
            _ => false,
        }
    }

    /// The field is a repeat.
    pub(crate) fn is_array(&self) -> bool {
        matches!(self.node, LayoutNode::Array { .. })
    }

    /// Every leaf is a producer literal.
    pub(crate) fn literal_only(&self) -> bool {
        !self.leaves.is_empty() && self.leaves.iter().all(literal_only)
    }

    /// The record fields of an array item record.
    pub(crate) fn item_fields(&self) -> Option<Vec<Field<'a>>> {
        match self.node {
            LayoutNode::Array { item } => match item.as_ref() {
                LayoutNode::Record { fields } => Some(split_fields(fields, self.leaves)),
                _ => None,
            },
            _ => None,
        }
    }
}

/// The merged write-site slice of a field's non-marker leaves
/// ([`FinderCtx::value_slice`], `hops` definitions deep).
pub(crate) fn field_slice(ctx: &FinderCtx, field: &Field, hops: usize) -> ValueSlice {
    let mut slice = ValueSlice::default();
    for leaf in field.leaves {
        if literal_only(leaf) {
            continue;
        }
        if let Some(own) = ctx.value_slice(leaf, hops) {
            slice.absorb(&own);
        }
    }
    slice
}

/// The test of the innermost `CHOICE` guard shared by every leaf of `field`.
pub(crate) fn guard_test_of(ctx: &FinderCtx, field: &Field) -> Option<oxc_semantic::NodeId> {
    let first = field.leaves.first()?;
    let guard = first
        .write
        .guards
        .iter()
        .rev()
        .find(|guard| field.leaves.iter().all(|leaf| leaf.write.guards.contains(guard)))?;
    ctx.guard_test(*guard)
}

/// A card name with its rule over a field and the field's write-site slice.
pub(crate) type SliceRule<'r> = (&'static str, &'r dyn Fn(&Field, &ValueSlice) -> bool);

/// [`solve`] over write-site slices: every message's fields are sliced
/// `hops` deep and named by forced uniqueness of `rules`.
pub(crate) fn solve_by_slices(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
    hops: usize,
    rules: &[SliceRule],
) -> Result<FieldBinding, BindRefusal> {
    let mut binding = FieldBinding::new();
    for message in 0..layout.messages.len() {
        let fields = message_fields(view, layout, message)?;
        let slices: Vec<ValueSlice> = fields.iter().map(|field| field_slice(ctx, field, hops)).collect();
        let by_key: std::collections::HashMap<&str, &ValueSlice> =
            fields.iter().map(|field| field.key.as_str()).zip(slices.iter()).collect();
        let adapted: Vec<Box<dyn Fn(&Field) -> bool + '_>> = rules
            .iter()
            .map(|(_, rule)| {
                let by_key = &by_key;
                Box::new(move |field: &Field| by_key.get(field.key.as_str()).is_some_and(|slice| rule(field, slice)))
                    as Box<dyn Fn(&Field) -> bool>
            })
            .collect();
        let named: Vec<Rule> = rules
            .iter()
            .zip(&adapted)
            .map(|((name, _), rule)| (*name, rule.as_ref()))
            .collect();
        solve(message, &fields, &named, &mut binding)?;
    }
    Ok(binding)
}

/// A card name with its rule over a field.
pub(crate) type Rule<'r> = (&'static str, &'r dyn Fn(&Field) -> bool);

/// Name `fields` from `rules` by forced uniqueness: repeatedly give a field
/// the one rule name that matches it and is not taken yet. Refuses when a
/// field matches no free name or several names remain for some field.
pub(crate) fn solve(
    message: usize,
    fields: &[Field],
    rules: &[Rule],
    binding: &mut FieldBinding,
) -> Result<(), BindRefusal> {
    let candidates: Vec<Vec<&'static str>> = fields
        .iter()
        .map(|field| {
            rules
                .iter()
                .filter(|(_, rule)| rule(field))
                .map(|(name, _)| *name)
                .collect()
        })
        .collect();
    let mut assigned: Vec<Option<&'static str>> = vec![None; fields.len()];
    loop {
        let mut progress = false;
        for (index, field) in fields.iter().enumerate() {
            if assigned[index].is_some() {
                continue;
            }
            let free: Vec<&'static str> = candidates[index]
                .iter()
                .copied()
                .filter(|name| !assigned.contains(&Some(*name)))
                .collect();
            match free.as_slice() {
                [] => {
                    return Err(BindRefusal::new(format!(
                        "message {message}: field {} matches no free card field",
                        field.key
                    )))
                }
                [name] => {
                    assigned[index] = Some(*name);
                    progress = true;
                }
                _ => {}
            }
        }
        if assigned.iter().all(Option::is_some) {
            break;
        }
        if !progress {
            let open: Vec<&str> = fields
                .iter()
                .zip(&assigned)
                .filter(|(_, name)| name.is_none())
                .map(|(field, _)| field.key.as_str())
                .collect();
            return Err(BindRefusal::new(format!(
                "message {message}: fields {open:?} match several card fields (ambiguous)"
            )));
        }
    }
    for (field, name) in fields.iter().zip(assigned) {
        binding.insert(message, field.key.clone(), name.unwrap_or_default());
    }
    Ok(())
}

/// Bind every message of `layout` with the same rules.
pub(crate) fn solve_all_messages(view: &SlotView, layout: &SignalLayout, rules: &[Rule]) -> Result<FieldBinding, BindRefusal> {
    let mut binding = FieldBinding::new();
    for message in 0..layout.messages.len() {
        let fields = message_fields(view, layout, message)?;
        solve(message, &fields, rules, &mut binding)?;
    }
    Ok(binding)
}
