//! `CanvasReadbackProbe`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, LayoutField, LayoutNode, LeafKind, LeafView, SignalLayout, SlotView};
use crate::signals::analysis::{layout_nodes::*, render_probes::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `CanvasReadbackProbe`. Card: [`CARD`].
///
/// Shape: a scalar seed and record count around one repeat of per-scene
/// records (hash-or-null choice, two colour histories `R{N N N N}` with their
/// counts, two new-colour counts). Evidence: the slot reads pixels back
/// (`getImageData`) and round-trips them through an image (`drawImage`), or
/// carries the `canvas_probe_image_load_handler` role.
pub(crate) fn recognize_canvas_readback_probe(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    claim_for(view, ctx, "CanvasReadbackProbe")
}

/// Records are bound by what each value reads, never by position:
///
/// * the two histories are told apart by the readback that feeds them: the
///   producer's history recorder is called with a pixel slice of a
///   `getImageData` result and the history (`record.h`); a history fed from a
///   function that also draws an image onto a canvas (`drawImage`, the image
///   onload round trip) is the round-trip history, the other the direct one;
/// * each count, colour list and new-colour count reads `record.h.…`, so its
///   first member after the record index names its history;
/// * each colour component is `colour[k]` with a literal `k` (ImageData
///   RGBA order); the v_puma form writes the colour's `.length` first
///   (`pixelLength`) and then the four components.
pub(crate) fn bind_canvas_readback_probe(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    const SIGNAL: &str = "CanvasReadbackProbe";
    let feeds = readback_feeds(view, ctx).map_err(|reason| refuse(SIGNAL, reason))?;
    let mut binding = FieldBinding::new();
    for (message, layout_message) in layout.messages.iter().enumerate() {
        let fields = &layout_message.fields;
        let leaves = field_leaves(view, layout, message).ok_or_else(|| refuse(SIGNAL, "leaf alignment failed"))?;
        let records: Vec<usize> = (0..fields.len()).filter(|i| is_array(&fields[*i].node)).collect();
        let [records] = records.as_slice() else {
            return Err(refuse(SIGNAL, format!("message {message}: expected one record repeat")));
        };
        for (index, field) in fields.iter().enumerate() {
            if index == *records {
                continue;
            }
            if !is_scalar(&field.node) {
                return Err(refuse(SIGNAL, format!("{}: top-level field is not a scalar", field.key)));
            }
            let length = leaves[index].iter().all(|leaf| path_ends_with_length(ctx, leaf));
            binding.insert(message, field.key.clone(), if length { "recordCount" } else { "seedFraction" });
        }
        let scalars: Vec<&str> = fields
            .iter()
            .filter_map(|field| binding.get(message, &field.key))
            .collect();
        if !scalars.contains(&"recordCount") || scalars.iter().filter(|name| **name == "recordCount").count() != 1 {
            return Err(refuse(SIGNAL, format!("message {message}: no single record count (`.length`)")));
        }
        if scalars.iter().filter(|name| **name == "seedFraction").count() > 1 {
            return Err(refuse(SIGNAL, format!("message {message}: more than one seed scalar")));
        }
        binding.insert(message, fields[*records].key.clone(), "records");
        let LayoutNode::Array { item } = &fields[*records].node else { unreachable!() };
        // A record whose whole body sits under the hash choice (`C{B tail |
        // N tail}` as the loop item itself) is the one-field record of that
        // choice.
        let choice_record;
        let record: &[LayoutField] = match item.as_ref() {
            LayoutNode::Record { fields } => fields,
            LayoutNode::Choice { .. } => {
                choice_record = [LayoutField { key: format!("{}.item", fields[*records].key), node: item.as_ref().clone() }];
                &choice_record
            }
            _ => return Err(refuse(SIGNAL, "scene record is not a record")),
        };
        let parts = split_leaves(&leaves[*records], record).ok_or_else(|| refuse(SIGNAL, "record leaf alignment failed"))?;
        bind_readback_record(ctx, &feeds, message, record, &parts, &mut binding)
            .map_err(|reason| refuse(SIGNAL, reason))?;
    }
    Ok(binding)
}

/// The history properties fed by the direct readback and by the image round
/// trip.
struct ReadbackFeeds {
    direct: String,
    round_trip: String,
}

/// Split the leaves of one record iteration by record field.
fn split_leaves<'v>(leaves: &[&'v LeafView], fields: &[LayoutField]) -> Option<Vec<Vec<&'v LeafView>>> {
    let mut cursor = 0;
    let mut out = Vec::new();
    for field in fields {
        let count = node_leaf_count(&field.node);
        out.push(leaves.get(cursor..cursor + count)?.to_vec());
        cursor += count;
    }
    (cursor == leaves.len()).then_some(out)
}

fn path_ends_with_length(ctx: &FinderCtx, leaf: &LeafView) -> bool {
    matches!(leaf_path(ctx, leaf).last(), Some(crate::signals::analysis::write_sites::PathStep::Property(name)) if name == "length")
}

/// The first member read after the record element (only index steps before
/// it): the history the value belongs to.
fn path_history(path: &[crate::signals::analysis::write_sites::PathStep]) -> Option<&str> {
    use crate::signals::analysis::write_sites::PathStep;
    path.iter().find_map(|step| match step {
        PathStep::Property(name) => Some(Some(name.as_str())),
        PathStep::Index(_) | PathStep::Dynamic => None,
    })?
}

/// The history every leaf of `leaves` reads, when they agree and it is one
/// of the two fed histories: `Some(true)` direct, `Some(false)` round trip.
fn leaves_history(ctx: &FinderCtx, feeds: &ReadbackFeeds, leaves: &[&LeafView]) -> Result<bool, String> {
    let mut found = std::collections::BTreeSet::new();
    for leaf in leaves {
        let path = leaf_path(ctx, leaf);
        match path_history(&path) {
            Some(name) if name == feeds.direct => found.insert(true),
            Some(name) if name == feeds.round_trip => found.insert(false),
            _ => return Err("a history value does not read a fed history".to_string()),
        };
    }
    match found.into_iter().collect::<Vec<_>>().as_slice() {
        [direct] => Ok(*direct),
        _ => Err("history values read both histories".to_string()),
    }
}

/// The component index a colour leaf reads (`colour[k]`).
fn component_index(ctx: &FinderCtx, leaf: &LeafView) -> Option<usize> {
    match leaf_path(ctx, leaf).last() {
        Some(crate::signals::analysis::write_sites::PathStep::Index(k)) if (0.0..4.0).contains(k) && k.fract() == 0.0 => Some(*k as usize),
        _ => None,
    }
}

const CHANNELS: [&str; 4] = ["red", "green", "blue", "alpha"];

fn bind_readback_record(
    ctx: &FinderCtx,
    feeds: &ReadbackFeeds,
    message: usize,
    record: &[LayoutField],
    parts: &[Vec<&LeafView>],
    binding: &mut FieldBinding,
) -> Result<(), String> {
    let hashes: Vec<usize> = (0..record.len()).filter(|i| matches!(record[*i].node, LayoutNode::Choice { .. })).collect();
    let [hash] = hashes.as_slice() else { return Err("expected one hash-or-null choice per record".into()) };
    // A choice that is the loop item itself has no key of its own.
    if !record[*hash].key.ends_with(".item") {
        binding.insert(message, record[*hash].key.clone(), "dataUrlHash");
    }
    let LayoutNode::Choice { arms } = &record[*hash].node else { unreachable!() };
    if !arms.iter().all(|arm| matches!(arm.node, LayoutNode::Record { .. })) {
        let fields: Vec<&LayoutField> = record.iter().collect();
        return bind_readback_fields(ctx, feeds, message, &fields, parts, Some(*hash), binding);
    }
    // The record's tail after the hash choice was copied into both arms
    // (`C{B tail | N tail}`): each arm is the hash (the null-marker byte, or
    // the hash number) followed by the rest of the record. Bind each arm as
    // the record it stands for.
    let heads: Vec<Option<LeafKind>> = arms
        .iter()
        .map(|arm| match &arm.node {
            LayoutNode::Record { fields } => match fields.first().map(|field| &field.node) {
                // The marker literal is the written byte, or the static XOR
                // operand of a keystream byte read through a local
                // (`k = ks(); buf[i] = k ^ 130`).
                Some(LayoutNode::Leaf(leaf))
                    if leaf.kind == LeafKind::Byte
                        && (leaf.byte.is_some_and(|b| b >= 128)
                            || (leaf.byte.is_none() && leaf.xor.is_some_and(|x| x >= 128.0 && x < 256.0 && x.fract() == 0.0))) =>
                {
                    Some(LeafKind::Byte)
                }
                Some(LayoutNode::Leaf(leaf)) if leaf.kind == LeafKind::Number => Some(LeafKind::Number),
                _ => None,
            },
            _ => None,
        })
        .collect();
    if heads.len() != 2 || !heads.contains(&Some(LeafKind::Byte)) || !heads.contains(&Some(LeafKind::Number)) {
        return Err("hash choice arms are not `null marker, …` and `hash, …`".into());
    }
    let mut cursor = 0;
    for arm in arms {
        let LayoutNode::Record { fields: arm_fields } = &arm.node else { unreachable!() };
        let count = node_leaf_count(&arm.node);
        let arm_leaves = parts[*hash].get(cursor..cursor + count).ok_or("hash arm leaf alignment failed")?;
        cursor += count;
        let arm_parts = split_leaves(arm_leaves, arm_fields).ok_or("hash arm leaf alignment failed")?;
        let mut fields: Vec<&LayoutField> = record[..*hash].iter().collect();
        let mut field_parts: Vec<Vec<&LeafView>> = parts[..*hash].to_vec();
        binding.insert(message, arm_fields[0].key.clone(), "dataUrlHash");
        fields.extend(arm_fields.iter());
        field_parts.extend(arm_parts);
        fields.extend(record[*hash + 1..].iter());
        field_parts.extend(parts[*hash + 1..].iter().cloned());
        bind_readback_fields(ctx, feeds, message, &fields, &field_parts, Some(*hash), binding)?;
    }
    Ok(())
}

/// Bind one scene record given as its fields (the hash at `hash`, already
/// named) and their leaves.
fn bind_readback_fields(
    ctx: &FinderCtx,
    feeds: &ReadbackFeeds,
    message: usize,
    record: &[&LayoutField],
    parts: &[Vec<&LeafView>],
    hash: Option<usize>,
    binding: &mut FieldBinding,
) -> Result<(), String> {
    let hash = hash.ok_or("no hash field")?;
    let mut pairs = Vec::new();
    for (index, field) in record.iter().enumerate() {
        if is_array(&field.node) {
            if index == 0 || !is_scalar(&record[index - 1].node) {
                return Err("colour list without a count".into());
            }
            pairs.push((index - 1, index));
        }
    }
    let mut lists = Vec::new();
    for (count, list) in &pairs {
        let direct = leaves_history(ctx, feeds, &parts[*list])?;
        if leaves_history(ctx, feeds, &parts[*count])? != direct || !parts[*count].iter().all(|l| path_ends_with_length(ctx, l)) {
            return Err(format!("{}: colour count does not read its list's length", record[*count].key));
        }
        let (count_name, list_name) =
            if direct { ("directColourCount", "directColours") } else { ("roundTripColourCount", "roundTripColours") };
        binding.insert(message, record[*count].key.clone(), count_name);
        binding.insert(message, record[*list].key.clone(), list_name);
        lists.push(direct);
        let LayoutNode::Array { item } = &record[*list].node else { unreachable!() };
        bind_colour(ctx, message, item, &parts[*list], binding)?;
    }
    lists.sort();
    if lists != [false, true] {
        return Err("expected one direct and one round-trip colour list".into());
    }
    let mut news = Vec::new();
    for index in 0..record.len() {
        if index == hash || pairs.iter().any(|(count, list)| index == *count || index == *list) {
            continue;
        }
        if !is_scalar(&record[index].node) {
            return Err(format!("{}: unexpected record field", record[index].key));
        }
        let direct = leaves_history(ctx, feeds, &parts[index]).map_err(|reason| format!("{}: {reason}", record[index].key))?;
        binding.insert(message, record[index].key.clone(), if direct { "directNewCount" } else { "roundTripNewCount" });
        news.push(direct);
    }
    news.sort();
    if news != [false, true] {
        return Err("expected one direct and one round-trip new-colour count".into());
    }
    Ok(())
}

/// One stored colour: four component leaves (`colour[k]` with a literal
/// `k`), optionally after `pixelLength`, or the v_puma loop form
/// `pixelLength, R{components}` (components written by a loop over the
/// colour, so the channels are not told apart).
fn bind_colour(
    ctx: &FinderCtx,
    message: usize,
    item: &LayoutNode,
    leaves: &[&LeafView],
    binding: &mut FieldBinding,
) -> Result<(), String> {
    let LayoutNode::Record { fields } = item else { return Err("colour is not a record".into()) };
    let parts = split_leaves(leaves, fields).ok_or("colour leaf alignment failed")?;
    let mut seen = [false; 4];
    for (field, leaves) in fields.iter().zip(&parts) {
        if is_scalar(&field.node) && leaves.iter().all(|leaf| path_ends_with_length(ctx, leaf)) {
            binding.insert(message, field.key.clone(), "pixelLength");
            continue;
        }
        if is_array(&field.node) {
            // The loop form (`for (k = 0; k < colour.length; k++) write(colour[k])`):
            // one repeated component after the colour's length, every leaf
            // reading `colour[k]` with a computed index.
            let LayoutNode::Array { item } = &field.node else { unreachable!() };
            let loop_form = fields.len() == 2
                && is_scalar(item)
                && binding.get(message, &fields[0].key) == Some("pixelLength")
                && !leaves.is_empty()
                && leaves
                    .iter()
                    .all(|leaf| matches!(leaf_path(ctx, leaf).last(), Some(crate::signals::analysis::write_sites::PathStep::Dynamic)));
            if !loop_form {
                return Err(format!("{}: colour component loop does not read `colour[k]` after its length", field.key));
            }
            binding.insert(message, field.key.clone(), "components");
            return Ok(());
        }
        let indices: Vec<Option<usize>> = leaves
            .iter()
            .map(|leaf| component_index(ctx, leaf))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        let (true, [Some(k)]) = (is_scalar(&field.node), indices.as_slice()) else {
            return Err(format!("{}: colour component does not read a literal RGBA index", field.key));
        };
        if std::mem::replace(&mut seen[*k], true) {
            return Err(format!("{}: component {k} written twice", field.key));
        }
        binding.insert(message, field.key.clone(), CHANNELS[*k]);
    }
    if seen != [true; 4] {
        return Err("colour does not write all four components".into());
    }
    Ok(())
}

/// Which history property each readback feeds: calls inside the slot's
/// producer that pass a `getImageData` pixel slice together with a member
/// `x.h` (the history recorder). A call whose innermost function also draws
/// an image (`drawImage`) feeds the round-trip history.
fn readback_feeds(view: &SlotView, ctx: &FinderCtx) -> Result<ReadbackFeeds, String> {
    use oxc_ast::ast::{Argument, Expression};
    use oxc_ast::AstKind;
    use oxc_span::GetSpan;
    let nodes = ctx.parse.nodes;
    let function_of = |mut id: oxc_semantic::NodeId| loop {
        let parent = nodes.parent_id(id);
        if parent == id {
            return None;
        }
        id = parent;
        if matches!(nodes.kind(id), AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)) {
            return Some(id);
        }
    };
    // The producer: the innermost function holding every write of the slot.
    let mut writes: Vec<oxc_span::Span> = Vec::new();
    for emission in view.emissions() {
        for leaf in &emission.leaves {
            writes.push(match leaf.write.helper_call {
                Some(call) => oxc_span::Span::new(call.start, call.end),
                None => leaf.write.span,
            });
        }
    }
    let first = writes.first().ok_or("no writes")?;
    let mut producer = ctx.node_with_span(*first).and_then(function_of).ok_or("no producer function")?;
    loop {
        let span = nodes.kind(producer).span();
        if writes.iter().all(|write| span.start <= write.start && write.end <= span.end) {
            break;
        }
        producer = function_of(producer).ok_or("no function holds every write")?;
    }
    let draws = |function: oxc_semantic::NodeId| {
        ctx.nodes_within(nodes.kind(function).span()).into_iter().any(|id| match nodes.kind(id) {
            AstKind::CallExpression(call) => matches!(
                call.callee.without_parentheses(),
                Expression::StaticMemberExpression(member) if member.property.name == "drawImage"
            ),
            _ => false,
        })
    };
    // The readbacks sit in the producer, or (an emission in its own
    // completion callback) in the nearest enclosing function that has any.
    let mut scope = producer;
    let mut direct = std::collections::BTreeSet::new();
    let mut round_trip = std::collections::BTreeSet::new();
    loop {
        for id in ctx.nodes_within(nodes.kind(scope).span()) {
            let AstKind::CallExpression(call) = nodes.kind(id) else { continue };
            // The recorder is called with exactly the pixel slice and the
            // history (`record(pixel, rec.h)`); a wire write or an inlined
            // push helper that reads pixels takes other arguments.
            if call.arguments.len() != 2 {
                continue;
            }
            let expression = |argument: &Argument| argument.as_expression().and_then(|e| ctx.node_with_span(e.span()));
            let pixel = call.arguments.iter().any(|argument| {
                expression(argument).is_some_and(|node| {
                    !matches!(nodes.kind(node), AstKind::StaticMemberExpression(_))
                        && ctx.slice_of(node, 3).calls_method("getImageData")
                })
            });
            if !pixel {
                continue;
            }
            let histories: Vec<String> = call
                .arguments
                .iter()
                .filter_map(|argument| match argument.as_expression()?.without_parentheses() {
                    Expression::StaticMemberExpression(member) => Some(member.property.name.to_string()),
                    _ => None,
                })
                .collect();
            let [history] = histories.as_slice() else { continue };
            let Some(function) = function_of(id) else { continue };
            if draws(function) { &mut round_trip } else { &mut direct }.insert(history.clone());
        }
        if !direct.is_empty() || !round_trip.is_empty() {
            break;
        }
        match function_of(scope) {
            Some(outer) => scope = outer,
            None => break,
        }
    }
    match (direct.len(), round_trip.len()) {
        (1, 1) if direct != round_trip => Ok(ReadbackFeeds {
            direct: direct.into_iter().next().unwrap(),
            round_trip: round_trip.into_iter().next().unwrap(),
        }),
        _ => Err(format!(
            "cannot tell the direct history from the round-trip history: direct feeds {direct:?}, image round-trip \
             feeds {round_trip:?}"
        )),
    }
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `CanvasReadbackProbe`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "CanvasReadbackProbe",
    summary: "Draws four simple scenes on a tiny 5x5 canvas: solid black, a black filled path, a fill with negative (transparent) alpha, and shadowed emoji text drawn outside the canvas. For each scene it reads the pixels back with getImageData, hashes the canvas's toDataURL() output, and also decodes that PNG through an Image, draws it onto a fresh canvas and reads the pixels again. It sends, per scene, the distinct pixel colours seen by each readback and how many there were; the order of a colour's components differs between builds. It is sent after the 4th image decodes, or when the drawing queue empties if that comes first.",
    why: "On an unmodified browser every readback should be one uniform opaque-black pixel, so extra or different colours likely reveal canvas noise injection (anti-fingerprinting farbling, privacy extensions) or non-standard rendering in headless or emulated stacks. The data-URL hash adds an encoder fingerprint.",
    sample_payload: Some(r#"{
  "records": [
    {
      "dataUrlHash": 640789961,
      "directColours": [
        {
          "alpha": 255,
          "blue": 0,
          "green": 0,
          "red": 0
        }
      ],
      "directNewCount": 1,
      "roundTripColours": [
        {
          "alpha": 255,
          "blue": 0,
          "green": 0,
          "red": 0
        }
      ],
      "roundTripNewCount": 1
    },
    {
      "dataUrlHash": 640789961,
      "directColours": [
        {
          "alpha": 255,
          "blue": 0,
          "green": 0,
          "red": 0
        }
      ],
      "directNewCount": 1,
      "roundTripColours": [
        {
          "alpha": 255,
          "blue": 0,
          "green": 0,
          "red": 0
        }
      ],
      "roundTripNewCount": 1
    },
    {
      "dataUrlHash": 640789961,
      "directColours": [
        {
          "alpha": 255,
          "blue": 0,
          "green": 0,
          "red": 0
        }
      ],
      "directNewCount": 1,
      "roundTripColours": [
        {
          "alpha": 255,
          "blue": 0,
          "green": 0,
          "red": 0
        }
      ],
      "roundTripNewCount": 1
    },
    {
      "dataUrlHash": 640789961,
      "directColours": [
        {
          "alpha": 255,
          "blue": 0,
          "green": 0,
          "red": 0
        }
      ],
      "directNewCount": 1,
      "roundTripColours": [
        {
          "alpha": 255,
          "blue": 0,
          "green": 0,
          "red": 0
        }
      ],
      "roundTripNewCount": 1
    }
  ],
  "seedFraction": 0.30215498399752305
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number(
            "seedFraction",
            "Fraction (0 to under 1) from the random state used to pick the scenes' random colours, so the server can rebuild them.",
        ),
        CardField::number("recordCount", "Number of scene records; always 4."),
        CardField::group("records", &[
            CardField::number(
                "dataUrlHash",
                "Signed 32-bit hash of the canvas's toDataURL() string for this scene, or a null marker if it was not computed (for example toDataURL failed).",
            ).nullable(),
            CardField::number(
                "directNewCount",
                "How many of the 25 direct-readback pixels were a new colour when read; equals the distinct colour count unless there are more than 6.",
            ),
            CardField::number(
                "roundTripColourCount",
                "Number of distinct colours stored from the image round-trip readback, 0 to 6 (0 if the image had not loaded yet).",
            ),
            CardField::group("roundTripColours", &[
                CardField::number(
                    "pixelLength",
                    "Number of components in the stored colour (always 4); only sent by builds that write it before the components.",
                ),
                CardField::number("red", "Red component, 0 to 255."),
                CardField::number("green", "Green component, 0 to 255."),
                CardField::number("alpha", "Alpha (opacity) component, 0 to 255."),
                CardField::number("blue", "Blue component, 0 to 255."),
                CardField::number(
                    "components",
                    "Colour component, 0 to 255; sent instead of red/green/alpha/blue by builds that write the components with a loop.",
                )
                .repeat("once per component of the stored colour (pixelLength times, ImageData RGBA order)"),
            ])
            .repeat("once per stored round-trip colour, up to 6"),
            CardField::number(
                "roundTripNewCount",
                "How many round-trip pixels were a new colour when read; same meaning as directNewCount.",
            ),
            CardField::number(
                "directColourCount",
                "Number of distinct colours stored from the direct readback, 0 to 6.",
            ),
            CardField::group("directColours", &[
                CardField::number(
                    "pixelLength",
                    "Number of components in the stored colour (always 4); only sent by builds that write it before the components.",
                ),
                CardField::number("red", "Red component, 0 to 255."),
                CardField::number("green", "Green component, 0 to 255."),
                CardField::number("alpha", "Alpha (opacity) component, 0 to 255."),
                CardField::number("blue", "Blue component, 0 to 255."),
                CardField::number(
                    "components",
                    "Colour component, 0 to 255; sent instead of red/green/alpha/blue by builds that write the components with a loop.",
                )
                .repeat("once per component of the stored colour (pixelLength times, ImageData RGBA order)"),
            ])
            .repeat("once per stored direct colour, up to 6, last seen first"),
        ])
        .repeat("once per scene, 4 times"),
    ]),
};
