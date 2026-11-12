//! Canvas, WebGL, audio, font and codec probes told apart by shape class and
//! slot-local evidence: each shape class lists its candidate signals with the
//! APIs, finder roles or provenance transforms that prove them, and the
//! candidate with the strictly closest evidence wins ([`classify`]); equal
//! evidence leaves the slot ambiguous.
//!
//! Used by: `CanvasReadbackProbe`, `CanvasScenePixelSamples`,
//! `CanvasToDataUrl`, `EmojiChannelSampleHalves`, `EmojiChannelSamples`,
//! `EmojiGlyphRgbaSamples`, `FontAvailability`, `FontMetrics`,
//! `OfflineAudioRenderSamples`, `VideoCanPlayType`, `WebGlContext`,
//! `WebGlRenderHash`.

use crate::signals::extract::contracts::SourceClass;
use super::shape_text::{compact_shapes, permutation_of, top_items};
use super::slot_closures::{self, any_api_depth, api_depth, guard_slices, value_slices, value_slices_within, LocalFacts, FED_DEPTH, MAX_EVIDENCE_DEPTH};
use super::{Claim, FinderCtx, LeafOp, SlotView};

/// One candidate signal of a shape class with its evidence.
pub(crate) struct Candidate {
    pub(crate) signal: &'static str,
    /// APIs the slot must touch (all of them).
    pub(crate) apis: &'static [&'static str],
    /// Package-finder roles that prove the signal on their own (depth 0).
    pub(crate) roles: &'static [&'static str],
    /// Deepest evidence accepted for this candidate.
    pub(crate) max_depth: usize,
    /// Value-provenance transforms that prove the signal on their own
    /// (depth 0): a written value derived from `.toDataURL()`.
    pub(crate) leaf_transforms: &'static [&'static str],
    /// The API must feed the written values: the value slices (definitions
    /// followed back from the repeated values) reach one of `apis`. A slot
    /// whose closures reach the API only through shared plumbing (a timing
    /// or entropy list filled next to a codec or audio probe) is not it.
    pub(crate) fed: bool,
}

/// The render-probe signals a slot is, by shape class and slot-local evidence. Within
/// one shape class the candidate with the strictly smallest evidence depth
/// wins; equal evidence returns every tied candidate (the slot is then
/// reported ambiguous rather than guessed). API evidence is read from the
/// value closure when it supports any candidate of the class, and from the
/// wider anchor closure only otherwise.
pub(crate) fn classify(view: &SlotView, ctx: &FinderCtx) -> Vec<(&'static str, String)> {
    let shapes = compact_shapes(view);
    if shapes.is_empty() {
        return Vec::new();
    }
    let candidates: &[Candidate] = shape_class(view, &shapes);
    if candidates.is_empty() {
        return Vec::new();
    }
    let local = slot_closures::facts(view, ctx);
    let roles = ctx.slot_roles(view);
    let mut proven: Vec<(usize, &'static str, String)> = Vec::new();
    for candidate in candidates {
        if let Some(role) = candidate.roles.iter().find(|role| roles.contains(role)) {
            proven.push((0, candidate.signal, format!("finder role {role}")));
            continue;
        }
        let transform = candidate.leaf_transforms.iter().find(|transform| {
            view.leaves().any(|leaf| leaf.transforms.iter().any(|t| t == *transform))
        });
        if let Some(transform) = transform {
            proven.push((0, candidate.signal, format!("value derived from {transform}")));
        }
    }
    // `all`: every API of the candidate is reached (closures); otherwise
    // any one of them (slices, which follow definitions only and so miss
    // statement calls such as `ctx.fillText(…)`).
    let slices = value_slices(view, ctx);
    let fed_slices = if candidates.iter().any(|candidate| candidate.fed) {
        value_slices_within(view, ctx, FED_DEPTH)
    } else {
        LocalFacts::default()
    };
    let by_api = |facts: &LocalFacts, closure: &str, all: bool| -> Vec<(usize, &'static str, String)> {
        candidates
            .iter()
            .filter(|candidate| !candidate.apis.is_empty())
            .filter(|candidate| {
                // Fed: the value slices reach the API, or the value roots'
                // own statements call every API of the candidate.
                !candidate.fed
                    || any_api_depth(&fed_slices, candidate.apis).is_some()
                    || api_depth(&local.value, candidate.apis) == Some(0)
            })
            .filter(|candidate| !proven.iter().any(|(_, signal, _)| *signal == candidate.signal))
            .filter_map(|candidate| {
                let depth = if all { api_depth(facts, candidate.apis)? } else { any_api_depth(facts, candidate.apis)? };
                (depth <= candidate.max_depth).then(|| {
                    (depth, candidate.signal, format!("{closure} closure reaches {:?} within {depth} hops", candidate.apis))
                })
            })
            .collect()
    };
    let mut evidence = by_api(&local.value, "value", true);
    if evidence.is_empty() {
        evidence = by_api(&slices, "value-slice", true);
    }
    if evidence.is_empty() {
        evidence = by_api(&local.anchor, "anchor", true);
    }
    // A list filled in another callback than the one that sends it (the
    // codec results stored into a producer-level list): the fed slices,
    // which follow the list's stores, are the values' only path to the API.
    if evidence.is_empty() {
        evidence = by_api(&fed_slices, "fed-slice", false);
    }
    if evidence.is_empty() {
        evidence = by_api(&guard_slices(view, ctx), "guard-slice", false);
    }
    // Candidates tied on closure evidence: the value slices of the
    // repeated values (definitions followed) break the tie when they reach
    // the APIs of some of them only, or of some at a smaller depth.
    if proven.is_empty() && closest(evidence.clone()).len() > 1 {
        let tied: Vec<(usize, &'static str, String)> = closest(evidence.clone())
            .into_iter()
            .filter_map(|(signal, text)| {
                let candidate = candidates.iter().find(|candidate| candidate.signal == signal)?;
                let depth = any_api_depth(&slices, candidate.apis)?;
                Some((depth, signal, format!("{text}; value slices reach one of {:?} within {depth} hops", candidate.apis)))
            })
            .collect();
        if !tied.is_empty() && closest(tied.clone()).len() == 1 {
            evidence = tied;
        }
    }
    let mut scored = proven;
    scored.extend(evidence);
    closest(scored)
        .into_iter()
        .map(|(signal, evidence)| (signal, format!("shape {:?}; {evidence}", shapes)))
        .collect()
}

/// The candidates with the smallest evidence depth; equal evidence keeps
/// every tied candidate, so the slot is reported ambiguous, not guessed.
pub(crate) fn closest(scored: Vec<(usize, &'static str, String)>) -> Vec<(&'static str, String)> {
    let Some(best) = scored.iter().map(|(depth, _, _)| *depth).min() else {
        return Vec::new();
    };
    scored
        .into_iter()
        .filter(|(depth, _, _)| *depth == best)
        .map(|(_, signal, evidence)| (signal, evidence))
        .collect()
}

/// `VideoCanPlayType`: codec probe results fed by `canPlayType`.
pub(crate) const VIDEO: Candidate = Candidate {
    signal: "VideoCanPlayType",
    apis: &["canPlayType"],
    roles: &[],
    max_depth: MAX_EVIDENCE_DEPTH,
    leaf_transforms: &[],
    fed: true,
};

/// `EmojiChannelSamples`: read pixels of drawn emoji glyphs.
pub(crate) const EMOJI_CHANNEL: Candidate = Candidate {
    signal: "EmojiChannelSamples",
    apis: &["getImageData", "fillText"],
    roles: &[],
    max_depth: MAX_EVIDENCE_DEPTH,
    leaf_transforms: &[],
    // The samples are read pixels; an entropy list filled next to the glyph
    // probe reaches it only through shared plumbing.
    fed: true,
};

/// `FontMetrics`: text-metric hashes of a delivered font list.
pub(crate) const FONT_METRICS: Candidate = Candidate {
    signal: "FontMetrics",
    apis: &["measureText", "actualBoundingBoxAscent"],
    roles: &["seeded_font_metrics_digest", "seeded_font_metrics_probe", "font_family_metrics", "font_metric_hash_mapper"],
    max_depth: 4,
    leaf_transforms: &[],
    fed: false,
};

/// `EmojiChannelSampleHalves`: two halves of read glyph pixels.
pub(crate) const EMOJI_HALVES: Candidate = Candidate {
    signal: "EmojiChannelSampleHalves",
    apis: &["getImageData"],
    roles: &[],
    // The only candidate of its shape class; the sampler callback pushes
    // through a copied value (`v = ctx.getImageData(…).data[c]; s = v;
    // push.call(list, s)`), one hop past the default.
    max_depth: 4,
    leaf_transforms: &[],
    fed: false,
};

/// `EmojiGlyphRgbaSamples`: RGBA rows of drawn glyphs.
pub(crate) const EMOJI_GLYPH: Candidate = Candidate {
    signal: "EmojiGlyphRgbaSamples",
    apis: &["getImageData", "fillText"],
    roles: &[],
    max_depth: MAX_EVIDENCE_DEPTH,
    leaf_transforms: &[],
    fed: false,
};

/// `CanvasScenePixelSamples`: pixel samples of a drawn scene.
pub(crate) const SCENE: Candidate = Candidate {
    signal: "CanvasScenePixelSamples",
    apis: &["getImageData"],
    roles: &[],
    max_depth: MAX_EVIDENCE_DEPTH,
    leaf_transforms: &[],
    fed: false,
};

/// `CanvasReadbackProbe`: pixels read back directly and through an image.
pub(crate) const READBACK: Candidate = Candidate {
    signal: "CanvasReadbackProbe",
    apis: &["getImageData", "drawImage"],
    roles: &["canvas_probe_image_load_handler"],
    max_depth: MAX_EVIDENCE_DEPTH,
    leaf_transforms: &[],
    fed: false,
};

/// `CanvasToDataUrl`: a value derived from `.toDataURL()`.
pub(crate) const DATA_URL: Candidate = Candidate {
    signal: "CanvasToDataUrl",
    apis: &[],
    roles: &[],
    max_depth: MAX_EVIDENCE_DEPTH,
    leaf_transforms: &[".toDataURL()"],
    fed: false,
};

/// `WebGlContext`: many `getParameter`-style fields.
pub(crate) const WEBGL_CONTEXT: Candidate = Candidate {
    signal: "WebGlContext",
    apis: &["getParameter", "getSupportedExtensions"],
    roles: &[],
    max_depth: MAX_EVIDENCE_DEPTH,
    leaf_transforms: &[],
    fed: false,
};

/// `WebGlRenderHash`: a shader render result with tag bytes.
pub(crate) const WEBGL_RENDER: Candidate = Candidate {
    signal: "WebGlRenderHash",
    apis: &["createShader", "shaderSource"],
    roles: &["webgl_fingerprint_probe"],
    max_depth: MAX_EVIDENCE_DEPTH,
    leaf_transforms: &[],
    fed: false,
};

/// `OfflineAudioRenderSamples`: rendered audio channel samples.
pub(crate) const AUDIO: Candidate = Candidate {
    signal: "OfflineAudioRenderSamples",
    apis: &["getChannelData"],
    roles: &[],
    max_depth: MAX_EVIDENCE_DEPTH,
    leaf_transforms: &[],
    fed: true,
};

/// `FontAvailability`: measured widths or presence bits of probed fonts.
pub(crate) const FONT_AVAILABILITY: Candidate = Candidate {
    signal: "FontAvailability",
    apis: &["measureText"],
    roles: &["font_metric_width_runner", "font_metrics_queue_empty"],
    max_depth: MAX_EVIDENCE_DEPTH,
    leaf_transforms: &[],
    fed: false,
};

/// The candidate signals of the slot's shape class.
pub(crate) fn shape_class(view: &SlotView, shapes: &[String]) -> &'static [Candidate] {
    let facts = LeafFacts {
        webgl: is_webgl_context(view),
        literal_tags: literal_tag_bytes(view),
        delivered_count: delivered_count(view),
    };
    shape_class_of(shapes, facts)
}

/// Leaf-level facts [`shape_class_of`] reads besides the shapes.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct LeafFacts {
    /// 20+ leaves with several strings in every emission.
    pub(crate) webgl: bool,
    /// Literal top-level tag bytes ([`literal_tag_bytes`]).
    pub(crate) literal_tags: bool,
    /// The leading count reads an array handed in from outside the producer
    /// (a subscriber parameter: provenance `Opaque`, no source table), as the
    /// font-metrics hash list arrives; local probe tables are `SourceTable`.
    pub(crate) delivered_count: bool,
}

/// Every emission's first top-level leaf is an opaque, table-free value.
pub(crate) fn delivered_count(view: &SlotView) -> bool {
    !view.emissions().is_empty()
        && view.emissions().iter().all(|emission| {
            emission.leaves.first().is_some_and(|leaf| {
                leaf.repeat_depth == 0
                    && leaf.classes.contains(&SourceClass::Opaque)
                    && !leaf.classes.contains(&SourceClass::SourceTable)
            })
        })
}

/// [`shape_class`] over the compact shapes plus the leaf-level facts.
pub(crate) fn shape_class_of(shapes: &[String], facts: LeafFacts) -> &'static [Candidate] {
    if shapes.is_empty() {
        return &[];
    }
    let all = |test: &dyn Fn(&str) -> bool| shapes.iter().all(|shape| test(shape));
    // count + one repeated number per item: a delivered hash list is the
    // font metrics; a local probe table is the codec codes or emoji samples.
    if facts.delivered_count && all(&|s| s == "N R{N}" || s == "N R{C{N|}}") {
        return &[FONT_METRICS];
    }
    if all(&|s| s == "N R{N}") {
        return &[VIDEO, EMOJI_CHANNEL];
    }
    if all(&|s| s == "N R{N} N R{N}") {
        return &[EMOJI_HALVES];
    }
    if all(&|s| s == "N R{N R{N}}") {
        return &[EMOJI_GLYPH];
    }
    if all(&|s| s == "N N R{N R{N}}" || s == "N R{N R{N}} N") {
        return &[SCENE];
    }
    if all(&is_readback_shape) {
        return &[READBACK];
    }
    if all(&|s| {
        permutation_of(s, &["S", "N", "N"]) || permutation_of(s, &["S", "B", "N"]) || permutation_of(s, &["S", "N"])
    }) && shapes.iter().any(|s| !s.contains('B'))
    {
        return &[DATA_URL];
    }
    if facts.webgl {
        return &[WEBGL_CONTEXT];
    }
    // Messages the contract pass left (partly) unresolved — the tracer's
    // packed-register buffers (`b = [[], []]; b[1][i] = …`) — say nothing
    // either way while the resolved ones outnumber them.
    let unresolved = |s: &str| top_items(s).iter().any(|item| item.contains('U') || item.contains('O'));
    let resolved: Vec<&String> = shapes.iter().filter(|s| !unresolved(s)).collect();
    if !resolved.is_empty()
        && resolved.len() > shapes.len() - resolved.len()
        && resolved.iter().all(|s| is_render_hash_shape(s))
        && facts.literal_tags
    {
        return &[WEBGL_RENDER];
    }
    if all(&|s| permutation_of(s, &["N", "N", "R{N}"])) {
        return &[AUDIO];
    }
    if all(&|s| permutation_of(s, &["N", "N", "N", "R{N}"]) || is_font_bits_shape(s)) {
        return &[FONT_AVAILABILITY];
    }
    &[]
}

/// One top-level repeat of records holding a hash choice and two colour
/// histories (`R{N N N N}`), plus the scalar seed/count fields.
pub(crate) fn is_readback_shape(shape: &str) -> bool {
    let items = top_items(shape);
    let repeats: Vec<&&str> = items.iter().filter(|item| item.starts_with("R{")).collect();
    let [record] = repeats.as_slice() else { return false };
    let scalars = items.iter().filter(|item| **item == "N").count();
    scalars + 1 == items.len()
        && (1..=2).contains(&scalars)
        && (record.matches("R{N N N N}").count() == 2 || record.matches("R{N R{N}}").count() == 2)
        && (record.contains("C{B|N}") || record.contains("C{N|B}"))
}

/// Three single fields: a kind byte, a status and a digest, each a byte,
/// number or `C{B|N}` choice, at least one of them a byte.
pub(crate) fn is_render_hash_shape(shape: &str) -> bool {
    let items = top_items(shape);
    items.len() == 3
        && items.iter().all(|item| matches!(*item, "B" | "N" | "C{B|N}" | "C{N|B}"))
        && items.iter().any(|item| *item == "B")
}

/// Every emission writes a literal top-level byte, and some emission writes
/// two (the record kind tag and the null marker of the absent field).
pub(crate) fn literal_tag_bytes(view: &SlotView) -> bool {
    let counts: Vec<usize> = view
        .emissions()
        .iter()
        .filter(|emission| !emission.shape.contains("UNRESOLVED") && !emission.shape.contains("OPAQUE"))
        .map(|emission| {
            emission
                .leaves
                .iter()
                .filter(|leaf| leaf.repeat_depth == 0 && leaf.choice_depth == 0 && leaf.op == LeafOp::Byte)
                .filter(|leaf| leaf.transforms.is_empty() && leaf.classes.iter().all(|c| *c == SourceClass::Literal))
                .count()
        })
        .collect();
    !counts.is_empty() && counts.iter().all(|count| *count >= 1) && counts.iter().any(|count| *count >= 2)
}

/// FontAvailability family B: count, packed presence bytes (a byte repeat
/// and the trailing partial byte), version.
pub(crate) fn is_font_bits_shape(shape: &str) -> bool {
    let items = top_items(shape);
    let bits = |item: &str| matches!(item, "R{B}" | "R{C{B|}}" | "R{C{|B}}");
    let partial = |item: &str| matches!(item, "B" | "C{B|}" | "C{|B}");
    items.iter().filter(|item| bits(item)).count() == 1
        && items.iter().filter(|item| **item == "N").count() == 2
        && items.iter().all(|item| *item == "N" || bits(item) || partial(item))
}

/// Many getParameter-style fields: at least 20 leaves, several strings.
pub(crate) fn is_webgl_context(view: &SlotView) -> bool {
    view.emissions().iter().all(|emission| emission.leaves.len() >= 20)
        && view.leaf_count(LeafOp::String) >= 5
}

/// The claim of `signal` on this slot when [`classify`] names it (noting ties).
pub(crate) fn claim_for(view: &SlotView, ctx: &FinderCtx, signal: &str) -> Option<Claim> {
    let found = classify(view, ctx);
    found
        .iter()
        .find(|(name, _)| *name == signal)
        .map(|(_, evidence)| {
            let mut lines = vec![evidence.clone()];
            if found.len() > 1 {
                lines.push(format!(
                    "tied with {:?}",
                    found.iter().map(|(name, _)| *name).filter(|name| *name != signal).collect::<Vec<_>>()
                ));
            }
            Claim::new(lines)
        })
}

#[cfg(test)]
mod tests {
    use super::{closest, shape_class_of, LeafFacts};

    fn class(shapes: &[&str]) -> Vec<&'static str> {
        class_with(shapes, LeafFacts::default())
    }

    fn class_with(shapes: &[&str], facts: LeafFacts) -> Vec<&'static str> {
        let shapes: Vec<String> = shapes.iter().map(|shape| shape.to_string()).collect();
        shape_class_of(&shapes, facts).iter().map(|c| c.signal).collect()
    }

    /// `count, R{N}` is shared by three signals; the two-halves layout is its
    /// own class (EmojiChannelSamples vs EmojiChannelSampleHalves).
    #[test]
    fn tie_emoji_samples_vs_halves() {
        assert_eq!(class(&["N R{N}"]), vec!["VideoCanPlayType", "EmojiChannelSamples"]);
        let delivered = LeafFacts { delivered_count: true, ..LeafFacts::default() };
        assert_eq!(class_with(&["N R{N}"], delivered), vec!["FontMetrics"]);
        assert_eq!(class_with(&["N R{C{N|}}"], delivered), vec!["FontMetrics"]);
        assert_eq!(class(&["N R{N} N R{N}"]), vec!["EmojiChannelSampleHalves"]);
        assert!(!class(&["N R{N}", "N R{N} N R{N}"]).contains(&"EmojiChannelSampleHalves"));
    }

    /// CanvasToDataUrl (`S N N`, fallback `S B N`, short `S N`) vs the scene
    /// samples (`N N R{N R{N}}`) vs the glyph rows (`N R{N R{N}}`).
    #[test]
    fn tie_data_url_vs_scene_vs_glyph() {
        assert_eq!(class(&["S N N", "S B N"]), vec!["CanvasToDataUrl"]);
        assert_eq!(class(&["N N S"]), vec!["CanvasToDataUrl"]);
        assert_eq!(class(&["S N"]), vec!["CanvasToDataUrl"]);
        // A fallback layout alone is not enough.
        assert!(class(&["S B N"]).is_empty());
        assert_eq!(class(&["N N R{N R{N}}"]), vec!["CanvasScenePixelSamples"]);
        assert_eq!(class(&["N R{N R{N}} N"]), vec!["CanvasScenePixelSamples"]);
        assert_eq!(class(&["N R{N R{N}}"]), vec!["EmojiGlyphRgbaSamples"]);
    }

    /// The direct and alternate readback records versus the scene samples.
    #[test]
    fn tie_readback_vs_scene() {
        assert_eq!(
            class(&["N R{C{B|N} N R{N N N N} N N R{N N N N} N} N"]),
            vec!["CanvasReadbackProbe"]
        );
        assert_eq!(
            class(&["N R{C{B|N} N R{N R{N}} N N N R{N R{N}}}"]),
            vec!["CanvasReadbackProbe"]
        );
        assert!(!class(&["N N R{N R{N N N N}}"]).contains(&"CanvasReadbackProbe"));
    }

    /// Audio (`seed, count, R{N}`) vs FontAvailability family A (one more
    /// scalar) and family B (packed bytes).
    #[test]
    fn tie_audio_vs_font_availability() {
        assert_eq!(class(&["N R{N} N"]), vec!["OfflineAudioRenderSamples"]);
        assert_eq!(class(&["N N R{N}"]), vec!["OfflineAudioRenderSamples"]);
        assert_eq!(class(&["N N N R{N}"]), vec!["FontAvailability"]);
        assert_eq!(class(&["N R{N} N N"]), vec!["FontAvailability"]);
        assert_eq!(class(&["N N R{C{B|}} C{B|}"]), vec!["FontAvailability"]);
        assert_eq!(class(&["N R{B} B N"]), vec!["FontAvailability"]);
    }

    /// WebGlRenderHash needs literal tag bytes; three computed bytes
    /// (BrowserApiProxyFlags) are not a render hash.
    #[test]
    fn tie_render_hash_vs_flag_bytes() {
        let tags = LeafFacts { literal_tags: true, ..LeafFacts::default() };
        assert_eq!(class_with(&["B B N", "C{B|N} B B"], tags), vec!["WebGlRenderHash"]);
        assert!(class(&["B B B"]).is_empty());
        let webgl = LeafFacts { webgl: true, ..LeafFacts::default() };
        assert_eq!(class_with(&["C{|B} C{N|B}"], webgl), vec!["WebGlContext"]);
    }

    /// Equal evidence keeps both candidates (reported ambiguous), the closer
    /// one wins otherwise.
    #[test]
    fn closest_keeps_ties() {
        let tied = closest(vec![(1, "EmojiChannelSamples", String::new()), (1, "VideoCanPlayType", String::new())]);
        assert_eq!(tied.len(), 2);
        let won = closest(vec![(2, "EmojiChannelSamples", String::new()), (1, "VideoCanPlayType", String::new())]);
        assert_eq!(won.iter().map(|(name, _)| *name).collect::<Vec<_>>(), vec!["VideoCanPlayType"]);
        assert!(closest(Vec::new()).is_empty());
    }
}
