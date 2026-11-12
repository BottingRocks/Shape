//! Layout fields paired with the [`Desc`] of their leaves in every emission
//! that renders the message ([`Field`]), and the [`Binder`] / [`Records`]
//! engine that names them: top-level fields by what their value is made of
//! (the event member a running mean averages, whether it is squared, the
//! comparison guarding a running min/max, the lifecycle callback that
//! assigns a phase timestamp), record fields by the object-literal label
//! they are read from.
//!
//! For binders whose field, record-field and section order is permuted per
//! build and whose fields share one kind. Used by: `AnimationFrameTimings`,
//! `CurrentScriptAsyncPolicyState`, `DeviceOrientation`,
//! `IntervalClockDriftStats`, `KeyboardEventSamples`, `KeystrokeDynamics`,
//! `LifecycleEntropySamples`, `LifecyclePhaseTimings`,
//! `LocalStoragePersistentId`, `MouseTravelAndClickCount`,
//! `NavigationTimingOffsets`, `PasteCountUnfocusedInputFlag`,
//! `PersistentStorageState`, `PonosXhrRoundTrip`, `ScrollOffsets`,
//! `TimestampsEpoch`, `UiEventSamplesMouse`, `UiEventSamplesTouch`,
//! `UiEventSamplesWheel`, `VisibilityEventLog`.

use std::collections::BTreeSet;

use oxc_semantic::SymbolId;

use super::layout_nodes::{leaf_kinds, kind_code, node_leaf_count, same_fields};
use super::value_facts::{self, slot_facts, Desc, SlotFacts};
use super::{BindRefusal, FieldBinding, FinderCtx, LayoutNode, LeafKind, SignalLayout, SlotView, WriteSite};

/// A record value that is a clock reading: a call with no host member read
/// besides the clock alias (`elapsed()`, `context.timeElapsedSinceInit()`).
pub(crate) fn is_clock_value(meaning: &Desc) -> bool {
    (meaning.program_call || !meaning.calls.is_empty() || meaning.has_call("now"))
        && !["pageX", "pageY", "clientX", "clientY", "keyCode", "which", "key", "force", "button", "length"]
            .iter()
            .any(|member| meaning.has_member(member))
}

/// The shape class of one layout field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Shape {
    Scalar(LeafKind),
    Array,
    Record,
    Choice,
    Other,
}

/// One layout field of one message, with the descriptions of its leaves in
/// every emission that renders the message.
#[derive(Clone, Debug)]
pub(crate) struct Field {
    pub(crate) key: String,
    pub(crate) shape: Shape,
    /// Leaf kinds of the subtree, with whether the leaf is a literal byte.
    pub(crate) kinds: Vec<(LeafKind, bool)>,
    /// Per occurrence: the descriptions of the subtree's leaves.
    pub(crate) occurrences: Vec<Vec<Desc>>,
    /// Per occurrence: the write sites of the subtree's leaves.
    pub(crate) sites: Vec<Vec<WriteSite>>,
    /// Record fields (of a record, or of the record item of an array).
    pub(crate) children: Vec<Field>,
    /// Per arm of a choice whose arms are records (`c ? (tag, a) : (b, a)`):
    /// the arm's record fields.
    pub(crate) arms: Vec<Vec<Field>>,
}

impl Field {
    /// The field of a layout field, without leaf descriptions yet.
    pub(crate) fn skeleton(field: &crate::signals::analysis::LayoutField) -> Self {
        let mut kinds = Vec::new();
        leaf_kinds(&field.node, &mut kinds);
        let (shape, children) = match &field.node {
            LayoutNode::Leaf(leaf) => (Shape::Scalar(leaf.kind), Vec::new()),
            LayoutNode::ByteSelect { .. } => (Shape::Scalar(LeafKind::Byte), Vec::new()),
            LayoutNode::Array { item } => (
                Shape::Array,
                match &**item {
                    LayoutNode::Record { fields } => fields.iter().map(Field::skeleton).collect(),
                    _ => Vec::new(),
                },
            ),
            LayoutNode::Record { fields } => (Shape::Record, fields.iter().map(Field::skeleton).collect()),
            LayoutNode::Choice { .. } => (Shape::Choice, Vec::new()),
            _ => (Shape::Other, Vec::new()),
        };
        let arms = match &field.node {
            LayoutNode::Choice { arms } if arms.iter().any(|arm| matches!(arm.node, LayoutNode::Record { .. })) => arms
                .iter()
                .map(|arm| match &arm.node {
                    LayoutNode::Record { fields } => fields.iter().map(Field::skeleton).collect(),
                    _ => Vec::new(),
                })
                .collect(),
            _ => Vec::new(),
        };
        Self {
            key: field.key.clone(),
            shape,
            kinds,
            occurrences: Vec::new(),
            sites: Vec::new(),
            children,
            arms,
        }
    }

    /// The value kind: the kind of the non-literal leaves (a null-tag
    /// choice of a number is a number).
    pub(crate) fn kind(&self) -> Option<LeafKind> {
        let live: BTreeSet<u8> = self
            .kinds
            .iter()
            .filter(|(_, literal)| !literal)
            .map(|(kind, _)| kind_code(*kind))
            .collect();
        match live.len() {
            0 if !self.kinds.is_empty() => Some(LeafKind::Byte),
            1 => self.kinds.iter().find(|(_, literal)| !literal).map(|(kind, _)| *kind),
            _ => None,
        }
    }

    /// The backward slices of every `CHOICE` guard test above the field's
    /// write sites, merged.
    pub(crate) fn guard_slice(&self, ctx: &FinderCtx, hops: usize) -> crate::signals::analysis::write_sites::ValueSlice {
        let mut out = crate::signals::analysis::write_sites::ValueSlice::default();
        for site in self.sites.iter().flatten() {
            for guard in &site.guards {
                if let Some(test) = ctx.guard_test(*guard) {
                    out.absorb(&ctx.slice_of(test, hops));
                }
            }
        }
        out
    }

    /// Bindings read by the caller-side values of the field's non-literal
    /// leaves, `hops` definitions deep.
    pub(crate) fn value_symbols(&self, ctx: &FinderCtx, hops: usize) -> BTreeSet<SymbolId> {
        let mut out = BTreeSet::new();
        for (sites, descs) in self.sites.iter().zip(&self.occurrences) {
            for (site, desc) in sites.iter().zip(descs) {
                if desc.literal_only() {
                    continue;
                }
                if let Some(value) = ctx.caller_write_value_node(site) {
                    out.extend(ctx.slice_of(value, hops).symbols.into_iter().map(|(symbol, _)| symbol));
                }
            }
        }
        out
    }

    /// The leaf kinds of the subtree.
    pub(crate) fn leaf_kinds(&self) -> Vec<LeafKind> {
        self.kinds.iter().map(|(kind, _)| *kind).collect()
    }

    /// Every occurrence reads nothing but literals.
    pub(crate) fn literal_only(&self) -> bool {
        self.occurrences.iter().flatten().all(Desc::literal_only)
    }

    /// Every leaf is a literal tag byte (`130`, a null marker).
    pub(crate) fn is_tag(&self) -> bool {
        !self.kinds.is_empty() && self.kinds.iter().all(|(_, literal)| *literal)
    }

    /// The merged description of every non-literal leaf (all leaves when
    /// every leaf is a literal).
    pub(crate) fn merged(&self) -> Desc {
        let mut desc = Desc::default();
        let live: Vec<&Desc> = self.occurrences.iter().flatten().filter(|d| !d.literal_only()).collect();
        let chosen: Vec<&Desc> = if live.is_empty() {
            self.occurrences.iter().flatten().collect()
        } else {
            live
        };
        for one in chosen {
            desc.merge(one.clone());
        }
        desc
    }

    /// Per occurrence, the merged description of its non-literal leaves.
    pub(crate) fn per_occurrence(&self) -> Vec<Desc> {
        self.occurrences
            .iter()
            .map(|leaves| {
                let mut desc = Desc::default();
                let live: Vec<&Desc> = leaves.iter().filter(|d| !d.literal_only()).collect();
                let chosen: Vec<&Desc> = if live.is_empty() { leaves.iter().collect() } else { live };
                for one in chosen {
                    desc.merge(one.clone());
                }
                desc
            })
            .collect()
    }

    /// The record label this field is read from (`rec.k`).
    pub(crate) fn own_label(&self) -> Option<String> {
        let merged = self.merged();
        merged.labels.first().cloned()
    }

    /// The one `rec.k[i]` label this field is read from.
    pub(crate) fn indexed_label(&self) -> Option<(String, u32)> {
        let merged = self.merged();
        match merged.indexed.as_slice() {
            [one] => Some(one.clone()),
            _ => None,
        }
    }
}

/// Append one emission's leaf descriptions and sites to `fields`; false when they do not cover the layout.
pub(crate) fn add_occurrence(fields: &mut [Field], nodes: &[crate::signals::analysis::LayoutField], descs: &[Desc], sites: &[WriteSite]) -> bool {
    let mut cursor = 0;
    for (field, node) in fields.iter_mut().zip(nodes) {
        let count = node_leaf_count(&node.node);
        let (Some(slice), Some(writes)) = (descs.get(cursor..cursor + count), sites.get(cursor..cursor + count)) else {
            return false;
        };
        cursor += count;
        field.occurrences.push(slice.to_vec());
        field.sites.push(writes.to_vec());
        match &node.node {
            LayoutNode::Record { fields: inner } => {
                add_occurrence(&mut field.children, inner, slice, writes);
            }
            LayoutNode::Array { item } => {
                if let LayoutNode::Record { fields: inner } = &**item {
                    add_occurrence(&mut field.children, inner, slice, writes);
                }
            }
            LayoutNode::Choice { arms } if !field.arms.is_empty() => {
                // Leaves run arm by arm.
                let mut start = 0;
                for (arm, fields) in arms.iter().zip(field.arms.iter_mut()) {
                    let count = node_leaf_count(&arm.node);
                    if let (LayoutNode::Record { fields: inner }, Some(part), Some(part_sites)) =
                        (&arm.node, slice.get(start..start + count), writes.get(start..start + count))
                    {
                        add_occurrence(fields, inner, part, part_sites);
                    }
                    start += count;
                }
            }
            _ => {}
        }
    }
    cursor == descs.len()
}

/// Record fields with every record-armed choice replaced by one field per
/// arm position (the arms' fields at that position, merged), and for each
/// expanded field the keys it stands for: `(choice key, arm keys)`.
pub(crate) fn expand_arms(fields: &[Field]) -> Option<(Vec<Field>, Vec<Option<(String, Vec<String>)>>)> {
    let mut out = Vec::new();
    let mut origin = Vec::new();
    for field in fields {
        if field.arms.is_empty() {
            out.push(field.clone());
            origin.push(None);
            continue;
        }
        let width = field.arms[0].len();
        if width == 0 || field.arms.iter().any(|arm| arm.len() != width) {
            return None;
        }
        for position in 0..width {
            let parts: Vec<&Field> = field.arms.iter().map(|arm| &arm[position]).collect();
            let mut merged = Field {
                key: parts[0].key.clone(),
                shape: Shape::Other,
                kinds: Vec::new(),
                occurrences: Vec::new(),
                sites: Vec::new(),
                children: Vec::new(),
                arms: Vec::new(),
            };
            for part in &parts {
                if !part.children.is_empty() || !part.arms.is_empty() {
                    return None;
                }
                merged.kinds.extend(part.kinds.iter().copied());
                merged.occurrences.extend(part.occurrences.iter().cloned());
                merged.sites.extend(part.sites.iter().cloned());
            }
            merged.shape = match merged.kind() {
                Some(kind) => Shape::Scalar(kind),
                None => Shape::Other,
            };
            out.push(merged);
            origin.push(Some((field.key.clone(), parts.iter().map(|part| part.key.clone()).collect())));
        }
    }
    Some((out, origin))
}

/// Names the fields of a slot's layout from its structural facts.
pub(crate) struct Binder<'l> {
    pub(crate) layout: &'l SignalLayout,
    pub(crate) facts: std::rc::Rc<SlotFacts>,
    pub(crate) messages: Vec<Vec<Field>>,
    pub(crate) binding: FieldBinding,
}

impl<'l> Binder<'l> {
    /// The fields of every message of `layout` with their leaf descriptions.
    pub(crate) fn new(view: &SlotView, layout: &'l SignalLayout, ctx: &FinderCtx) -> Result<Self, BindRefusal> {
        let facts = slot_facts(view, ctx);
        let mut messages: Vec<Vec<Field>> = layout
            .messages
            .iter()
            .map(|message| message.fields.iter().map(Field::skeleton).collect())
            .collect();
        for emission in &layout.emissions {
            let Some(Some(descs)) = facts.emissions.get(emission.emission) else {
                continue;
            };
            for (part_index, (_, part)) in emission.parts.iter().enumerate() {
                let count: usize = part.fields.iter().map(|field| node_leaf_count(&field.node)).sum();
                // A part's leaves: one run of the emission, or (after a split
                // choice) its arm's run followed by the shared trailing items.
                let indices = emission.part_leaves(part_index);
                if indices.len() != count {
                    break;
                }
                let Some(part_descs) = indices.iter().map(|leaf| descs.get(*leaf).cloned()).collect::<Option<Vec<Desc>>>()
                else {
                    break;
                };
                let slice = part_descs.as_slice();
                let Some(message) = layout
                    .messages
                    .iter()
                    .position(|message| {
                        message.mask == part.mask
                            && part.when.iter().all(|when| message.when.contains(when))
                            && same_fields(&message.fields, &part.fields)
                    })
                else {
                    continue;
                };
                let Some(sites) = view
                    .emissions()
                    .get(emission.emission)
                    .and_then(|leaves| indices.iter().map(|leaf| leaves.leaves.get(*leaf).map(|leaf| leaf.write.clone())).collect::<Option<Vec<WriteSite>>>())
                else {
                    continue;
                };
                add_occurrence(&mut messages[message], &part.fields, slice, &sites);
            }
        }
        for (index, fields) in messages.iter().enumerate() {
            if fields.iter().any(|field| field.occurrences.is_empty()) {
                return Err(BindRefusal::new(format!(
                    "message {index}: the write sites could not be matched to the layout leaves"
                )));
            }
        }
        Ok(Self {
            layout,
            facts,
            messages,
            binding: FieldBinding::new(),
        })
    }

    /// Top-level field count per message.
    pub(crate) fn top_count(&self) -> Vec<usize> {
        self.messages.iter().map(Vec::len).collect()
    }

    /// Name every top-level field of every message with `classify`.
    ///
    /// A placeholder message (every unnamed field a literal, as the startup
    /// record of a statistics signal) takes the names of a fully named
    /// message with the same field shapes, position by position, when every
    /// field it could name itself agrees.
    pub(crate) fn top(&mut self, mut classify: impl FnMut(&Field) -> Option<&'static str>) -> Result<(), BindRefusal> {
        self.top_at(|_, fields, index| classify(&fields[index]))
    }

    /// [`Self::top`] with the field's message and position (`classify(message,
    /// fields, index)`, messages in order, fields in order).
    pub(crate) fn top_at(&mut self, mut classify: impl FnMut(usize, &[Field], usize) -> Option<&'static str>) -> Result<(), BindRefusal> {
        let mut named: Vec<Vec<Option<&'static str>>> = Vec::new();
        for (message, fields) in self.messages.iter().enumerate() {
            let mut used = BTreeSet::new();
            let mut names = Vec::new();
            for (index, _field) in fields.iter().enumerate() {
                let name = classify(message, fields, index);
                if let Some(name) = name {
                    if !used.insert(name) {
                        return Err(BindRefusal::new(format!("message {message}: two fields classify as {name}")));
                    }
                }
                names.push(name);
            }
            named.push(names);
        }
        let complete: Vec<usize> = (0..named.len()).filter(|m| named[*m].iter().all(Option::is_some)).collect();
        for message in 0..named.len() {
            if named[message].iter().all(Option::is_some) {
                continue;
            }
            let fields = &self.messages[message];
            let placeholder = fields
                .iter()
                .zip(&named[message])
                .all(|(field, name)| name.is_some() || field.literal_only());
            if !placeholder {
                continue;
            }
            let donor = complete.iter().copied().find(|other| {
                let others = &self.messages[*other];
                others.len() == fields.len()
                    && others.iter().zip(fields).all(|(a, b)| a.shape == b.shape && a.kind() == b.kind())
                    && named[*other].iter().zip(&named[message]).all(|(a, b)| b.is_none() || a == b)
            });
            if let Some(donor) = donor {
                named[message] = named[donor].clone();
            }
        }
        for (message, names) in named.iter().enumerate() {
            for (field, name) in self.messages[message].iter().zip(names) {
                if let Some(name) = name {
                    self.binding.insert(message, field.key.clone(), *name);
                }
            }
        }
        Ok(())
    }

    /// Name every top-level field from each occurrence separately; all
    /// occurrences with a name must agree.
    pub(crate) fn top_each(&mut self, classify: impl Fn(&Desc) -> Option<&'static str>) -> Result<(), BindRefusal> {
        self.top(|field| {
            let names: BTreeSet<&'static str> = field.per_occurrence().iter().filter_map(&classify).collect();
            match names.len() {
                1 => names.into_iter().next(),
                _ => None,
            }
        })
    }

    /// Name sections of `(count, list)` pairs: the list by its section
    /// class, the number right before it as that section's count.
    pub(crate) fn top_sections(
        &mut self,
        section: impl Fn(&Field) -> Option<&'static str>,
        table: &[(&'static str, &'static str, &'static str)],
    ) -> Result<(), BindRefusal> {
        for (message, fields) in self.messages.iter().enumerate() {
            let mut used = BTreeSet::new();
            for (index, field) in fields.iter().enumerate() {
                if field.shape != Shape::Array {
                    continue;
                }
                let Some(class) = section(field) else {
                    continue;
                };
                let Some((_, count, list)) = table.iter().find(|(name, _, _)| *name == class) else {
                    continue;
                };
                if !used.insert(class) {
                    return Err(BindRefusal::new(format!("message {message}: two {class} sections")));
                }
                self.binding.insert(message, field.key.clone(), *list);
                if let Some(previous) = index.checked_sub(1).and_then(|i| fields.get(i)) {
                    if previous.shape == Shape::Scalar(LeafKind::Number) && previous.merged().has_member("length") {
                        self.binding.insert(message, previous.key.clone(), *count);
                    }
                }
            }
        }
        Ok(())
    }

    /// Name the record fields of every list of records.
    pub(crate) fn records(&mut self, mut each: impl FnMut(&mut Records, &SlotFacts) -> Result<(), BindRefusal>) -> Result<(), BindRefusal> {
        let Binder {
            messages,
            binding,
            facts,
            ..
        } = self;
        for (message, fields) in messages.iter().enumerate() {
            for field in fields {
                if field.children.is_empty() {
                    continue;
                }
                let mut records = Records {
                    binding,
                    message,
                    fields: &field.children,
                };
                each(&mut records, facts)?;
            }
        }
        Ok(())
    }

    /// The binding, once every field of every message is named.
    pub(crate) fn finish(self) -> Result<FieldBinding, BindRefusal> {
        let mut unbound = Vec::new();
        for message in 0..self.layout.messages.len() {
            for key in self.layout.keys(message) {
                if self.binding.get(message, &key).is_none() {
                    unbound.push(format!("{message}:{key}"));
                }
            }
        }
        if unbound.is_empty() {
            Ok(self.binding)
        } else {
            Err(BindRefusal::new(format!("no card name for field(s) {}", unbound.join(", "))))
        }
    }
}

/// The record fields of one list.
pub(crate) struct Records<'b> {
    pub(crate) binding: &'b mut FieldBinding,
    pub(crate) message: usize,
    pub(crate) fields: &'b [Field],
}

impl Records<'_> {
    /// Name record fields by `classify` alone.
    pub(crate) fn name_by_kind(&mut self, classify: impl Fn(&Field) -> Option<&'static str>) -> Result<(), BindRefusal> {
        let names: Vec<Option<&'static str>> = self.fields.iter().map(&classify).collect();
        self.apply(&names)
    }

    /// Name the record fields with `names` (one per field, distinct).
    pub(crate) fn apply(&mut self, names: &[Option<&'static str>]) -> Result<(), BindRefusal> {
        let mut used = BTreeSet::new();
        for (field, name) in self.fields.iter().zip(names) {
            if let Some(name) = name {
                if !used.insert(*name) {
                    return Err(BindRefusal::new(format!("record fields: two fields classify as {name}")));
                }
                self.binding.insert(self.message, field.key.clone(), *name);
            }
        }
        Ok(())
    }

    /// Name record fields from the object literal their labels are filled
    /// from: every literal holding all of the record's labels is tried; the
    /// names must be complete, distinct and the same for every literal that
    /// names them all.
    pub(crate) fn name_by_labels(
        &mut self,
        facts: &SlotFacts,
        classify: impl Fn(&Field, &Desc) -> Option<&'static str>,
    ) -> Result<(), BindRefusal> {
        let fields = self.fields;
        self.name_expanded(fields, facts, &classify)
    }

    /// Name `fields` by their labels; a record-armed choice is named
    /// position by position (each arm key takes its position's name) and
    /// the choice itself `pairedFields` (`x == null ? (tag, y) : (x, y)`).
    pub(crate) fn name_expanded(
        &mut self,
        fields: &[Field],
        facts: &SlotFacts,
        classify: &impl Fn(&Field, &Desc) -> Option<&'static str>,
    ) -> Result<(), BindRefusal> {
        self.name_expanded_in(fields, facts, classify, false)
    }

    /// [`Self::name_expanded`]; `nested` for the item records of a list
    /// held inside another record.
    fn name_expanded_in(
        &mut self,
        fields: &[Field],
        facts: &SlotFacts,
        classify: &impl Fn(&Field, &Desc) -> Option<&'static str>,
        nested: bool,
    ) -> Result<(), BindRefusal> {
        if fields.iter().all(|field| field.arms.is_empty()) {
            let names = label_names_in(fields, facts, classify, nested)?;
            return Records {
                binding: self.binding,
                message: self.message,
                fields,
            }
            .apply(&names);
        }
        let (expanded, origin) =
            expand_arms(fields).ok_or_else(|| BindRefusal::new("a record choice has arms of different widths"))?;
        let names = label_names_in(&expanded, facts, classify, nested)?;
        let distinct: BTreeSet<&str> = names.iter().flatten().copied().collect();
        if names.iter().any(Option::is_none) || distinct.len() != names.len() {
            return Err(BindRefusal::new("record fields under a choice: unnamed or duplicate"));
        }
        let mut choice_names: Vec<(String, Vec<&'static str>)> = Vec::new();
        for (name, origin) in names.iter().flatten().zip(&origin) {
            match origin {
                None => {}
                Some((choice, arm_keys)) => {
                    for key in arm_keys {
                        self.binding.insert(self.message, key.clone(), *name);
                    }
                    match choice_names.last_mut() {
                        Some((last, parts)) if last == choice => parts.push(name),
                        _ => choice_names.push((choice.clone(), vec![name])),
                    }
                }
            }
        }
        for (field, name) in expanded.iter().zip(&names).zip(&origin).filter(|(_, origin)| origin.is_none()).map(|(pair, _)| pair) {
            if let Some(name) = name {
                self.binding.insert(self.message, field.key.clone(), *name);
            }
        }
        // The choice itself is the structural pair: which record fields it
        // carries varies per build and is named by its arm fields.
        if choice_names.len() > 1 {
            return Err(BindRefusal::new("two record-armed choices in one record"));
        }
        for (choice, _) in choice_names {
            self.binding.insert(self.message, choice, "pairedFields");
        }
        Ok(())
    }

    /// Name the record fields of every list nested in this record.
    pub(crate) fn nested(&mut self, facts: &SlotFacts, classify: impl Fn(&Field, &Desc) -> Option<&'static str>) -> Result<(), BindRefusal> {
        for field in self.fields {
            if field.children.is_empty() {
                continue;
            }
            let mut inner = Records {
                binding: self.binding,
                message: self.message,
                fields: &field.children,
            };
            inner.name_expanded_in(&field.children, facts, &classify, true)?;
        }
        Ok(())
    }
}

/// Name record fields from the object literal that holds every record label.
/// For the item records of a nested list (`nested`), the
/// literals pushed into a list that is itself stored under a key of another
/// record literal (`items.push({..}); rec.k = items`) are tried first.
fn label_names_in(
    fields: &[Field],
    facts: &SlotFacts,
    classify: &impl Fn(&Field, &Desc) -> Option<&'static str>,
    nested: bool,
) -> Result<Vec<Option<&'static str>>, BindRefusal> {
    let labels: BTreeSet<String> = fields.iter().filter_map(Field::own_label).collect();
    if labels.is_empty() {
        return Err(BindRefusal::new("record fields carry no labels"));
    }
    let mut best: Option<Vec<Option<&'static str>>> = None;
    // Literals holding every label; those stored into the list the record
    // is read from (`ring.put({..})` / `ring.toArray()`) when any is.
    let containers: BTreeSet<_> = fields.iter().flat_map(|field| field.merged().containers).collect();
    let holding: Vec<&value_facts::RecordLiteral> = facts
        .literals
        .iter()
        .filter(|literal| labels.iter().all(|label| literal.fields.contains_key(label)))
        .collect();
    let (stored, rest): (Vec<&value_facts::RecordLiteral>, Vec<&value_facts::RecordLiteral>) = holding
        .iter()
        .copied()
        .partition(|literal| !literal.containers.is_disjoint(&containers));
    let held_lists: BTreeSet<_> = facts.literals.iter().flat_map(|literal| literal.held.iter().copied()).collect();
    let (inner, stored): (Vec<&value_facts::RecordLiteral>, Vec<&value_facts::RecordLiteral>) =
        stored.into_iter().partition(|literal| !literal.containers.is_disjoint(&held_lists));
    let (inner_rest, rest): (Vec<&value_facts::RecordLiteral>, Vec<&value_facts::RecordLiteral>) =
        rest.into_iter().partition(|literal| !literal.containers.is_disjoint(&held_lists));
    let inner: Vec<&value_facts::RecordLiteral> = inner.into_iter().chain(inner_rest).collect();
    // The literals stored into the record's list first; the others only
    // when those name nothing. Item literals of a list held inside another
    // record come first for a nested list's records and last otherwise.
    let tiers = if nested { [inner, stored, rest] } else { [stored, rest, inner] };
    for tier in tiers {
        let found = name_with(fields, &tier, classify, &mut best)?;
        if found.is_some() {
            return Ok(found.unwrap_or_default());
        }
    }
    Err({
        let detail = best
            .map(|names| {
                let named: Vec<String> = fields
                    .iter()
                    .zip(&names)
                    .map(|(field, name)| format!("{} (label {:?}) -> {}", field.key, field.own_label(), name.unwrap_or("?")))
                    .collect();
                format!("; best literal names {} (unnamed or duplicate)", named.join(", "))
            })
            .unwrap_or_default();
        BindRefusal::new(format!("no object literal names every record label {labels:?}{detail}"))
    })
}

/// Name `fields` from each literal of `tier`; every literal that names
/// them all must agree.
pub(crate) fn name_with(
    fields: &[Field],
    tier: &[&value_facts::RecordLiteral],
    classify: &impl Fn(&Field, &Desc) -> Option<&'static str>,
    best: &mut Option<Vec<Option<&'static str>>>,
) -> Result<Option<Vec<Option<&'static str>>>, BindRefusal> {
    let mut found: Option<Vec<Option<&'static str>>> = None;
    for literal in tier {
        let literal = &literal.fields;
        let names: Vec<Option<&'static str>> = fields
            .iter()
            .map(|field| {
                let meaning = match (field.indexed_label(), field.own_label()) {
                    (Some((label, index)), _) if literal.contains_key(&format!("{label}[{index}]")) => {
                        literal[&format!("{label}[{index}]")].clone()
                    }
                    (_, Some(label)) => literal.get(&label).cloned().unwrap_or_default(),
                    _ => Desc::default(),
                };
                classify(field, &meaning)
            })
            .collect();
        let distinct: BTreeSet<&str> = names.iter().flatten().copied().collect();
        if names.iter().any(Option::is_none) || distinct.len() != names.len() {
            if best.as_ref().is_none_or(|b| b.iter().flatten().count() < distinct.len()) {
                *best = Some(names);
            }
            continue;
        }
        match &found {
            None => found = Some(names),
            Some(previous) if *previous == names => {}
            Some(_) => return Err(BindRefusal::new("record labels name differently in two literals")),
        }
    }
    Ok(found)
}
