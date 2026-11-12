//! `MatchMedia`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, LayoutNode, LeafKind, SignalLayout, SlotView, ValueSlice};
use crate::signals::analysis::{shape_text::*, leaf_checks::*, field_solver::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `MatchMedia`. Card: [`CARD`].
pub(crate) fn recognize_match_media(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    let leaves = first_emission_leaves(view);
    // Most builds route the range probes through the shared p10/p11 media
    // prober, so only a few leaves carry `.matches` themselves.
    let matches = leaves.iter().filter(|leaf| has_transform(leaf, ".matches")).count();
    let prober = ctx.has_role(view, "media_feature_range_prober");
    // The slot's own writes inside the proven range prober / serializer: the
    // query results reach the wire through program calls, so no leaf need
    // carry the `matchMedia()` path itself.
    let serialized = prober && ctx.has_role(view, "media_feature_range_serializer");
    let wide = max_top_count(&view.shapes()) >= 40;
    if wide && (serialized || ((matches >= 8 || prober) && any_api(&leaves, "matchMedia()"))) {
        return claim(&["40+ fields", "values are matchMedia(...).matches", "media feature range prober"]);
    }
    // Without the prober roles: the write sites themselves pass 8+ distinct
    // media feature queries (`probe(win, "max-width", 600, "px")`, the
    // categorical feature names) as direct call arguments.
    if wide {
        let mut features = std::collections::BTreeSet::new();
        for leaf in &leaves {
            if let Some(slice) = ctx.value_slice(leaf, 3) {
                features.extend(nearest_feature(&slice, MEDIA_GROUPS));
                features.extend(nearest_feature(&slice, MEDIA_BOUNDARIES));
            }
        }
        if features.len() >= 8 {
            return claim(&["40+ fields", "write sites pass 8+ media feature queries"]);
        }
    }
    None
}

/// Categorical media features (query text) and their card group prefix.
const MEDIA_GROUPS: &[(&str, &str)] = &[
    ("grid", "grid"),
    ("overflow-inline", "overflowInline"),
    ("overflow-block", "overflowBlock"),
    ("any-pointer", "anyPointer"),
    ("pointer", "pointer"),
    ("any-hover", "anyHover"),
    ("hover", "hover"),
    ("prefers-contrast", "prefersContrast"),
    ("scripting", "scripting"),
    ("light-level", "lightLevel"),
    ("display-mode", "displayMode"),
    ("forced-colors", "forcedColors"),
    ("prefers-color-scheme", "prefersColorScheme"),
    ("color-gamut", "colorGamut"),
    ("inverted-colors", "invertedColors"),
    ("orientation", "orientation"),
    ("scan", "scan"),
    ("prefers-reduced-transparency", "prefersReducedTransparency"),
    ("prefers-reduced-motion", "prefersReducedMotion"),
    ("update", "update"),
];

/// Numeric media features (boundary searches) and their card field.
const MEDIA_BOUNDARIES: &[(&str, &str)] = &[
    ("max-monochrome", "maxMonochrome"),
    ("max-height", "maxHeight"),
    ("max-width", "maxWidth"),
    ("max-color", "maxColor"),
    ("max-color-index", "maxColorIndex"),
    ("max-resolution", "resolution"),
    ("-webkit-max-device-pixel-ratio", "resolution"),
];

/// Card names of a categorical group: count, bit loop, flush byte.
fn media_group_names(prefix: &str) -> Option<[&'static str; 3]> {
    macro_rules! groups {
        ($($prefix:literal),*) => {
            match prefix {
                $($prefix => Some([concat!($prefix, "Count"), concat!($prefix, "Bits"), concat!($prefix, "Flush")]),)*
                _ => None,
            }
        };
    }
    groups!(
        "grid", "overflowInline", "overflowBlock", "anyPointer", "pointer", "anyHover", "hover",
        "prefersContrast", "scripting", "lightLevel", "displayMode", "forcedColors", "prefersColorScheme",
        "colorGamut", "invertedColors", "orientation", "scan", "prefersReducedTransparency",
        "prefersReducedMotion", "update"
    )
}

/// The table entries whose query text the slice passes as a direct call
/// argument (`probe(win, "max-width", 600, "px")`), at the smallest
/// matching depth. Candidate value lists (`["none", "hover"]`) are not
/// direct arguments.
fn nearest_feature(slice: &ValueSlice, table: &[(&str, &'static str)]) -> Vec<&'static str> {
    let hits: Vec<(usize, &'static str)> = slice
        .arguments
        .iter()
        .filter_map(|(arg, depth)| table.iter().find(|(query, _)| query == arg).map(|(_, name)| (*depth, *name)))
        .collect();
    let Some(min) = hits.iter().map(|(depth, _)| *depth).min() else {
        return Vec::new();
    };
    let mut names: Vec<&'static str> = hits.into_iter().filter(|(depth, _)| *depth == min).map(|(_, name)| name).collect();
    names.sort_unstable();
    names.dedup();
    names
}

pub(crate) fn bind_match_media(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    // A categorical group is written as NUMBER(candidate count), a bit loop
    // (in-loop flush every 8 bits) and a trailing flush byte; the group is
    // named by the query feature its count and loop read. Numeric
    // boundaries are named by their query text, the aspect-ratio fraction
    // by the index it reads, and the (q: 1) byte by its query.
    let mut binding = FieldBinding::new();
    for message in 0..layout.messages.len() {
        let fields = message_fields(view, layout, message)?;
        let slices: Vec<ValueSlice> = fields.iter().map(|field| field_slice(ctx, field, 3)).collect();
        let mut names: Vec<Option<&'static str>> = vec![None; fields.len()];
        let refuse = |index: usize, why: String| {
            BindRefusal::new(format!("MatchMedia: message {message}: field {}: {why}", fields[index].key))
        };
        let mut index = 0;
        while index < fields.len() {
            let field = &fields[index];
            let group = field.is_plain(LeafKind::Number)
                && fields.get(index + 1).is_some_and(|bits| bits.is_array())
                && fields
                    .get(index + 2)
                    .is_some_and(|flush| matches!(flush.node, LayoutNode::Choice { .. }) && flush.writes(LeafKind::Byte));
            if group {
                let count = nearest_feature(&slices[index], MEDIA_GROUPS);
                let bits = nearest_feature(&slices[index + 1], MEDIA_GROUPS);
                let prefix = match (count.as_slice(), bits.as_slice()) {
                    ([one], [two]) if one == two => *one,
                    ([one], []) | ([], [one]) => *one,
                    _ => return Err(refuse(index, format!("group reads features {count:?} / {bits:?}"))),
                };
                let group_names = media_group_names(prefix).ok_or_else(|| refuse(index, format!("no group {prefix}")))?;
                for (offset, name) in group_names.into_iter().enumerate() {
                    names[index + offset] = Some(name);
                }
                index += 3;
                continue;
            }
            // A group of at most 8 candidates accumulates its bits without an
            // in-loop flush: count + trailing flush only, both reading the
            // same categorical feature.
            let short_group = field.is_plain(LeafKind::Number)
                && fields
                    .get(index + 1)
                    .is_some_and(|flush| matches!(flush.node, LayoutNode::Choice { .. }) && flush.writes(LeafKind::Byte));
            if short_group {
                let count = nearest_feature(&slices[index], MEDIA_GROUPS);
                let flush = nearest_feature(&slices[index + 1], MEDIA_GROUPS);
                // As for a full group, one side may lose the feature (a bit
                // loop printed as `for (;;) { if (i >= 0) { …; continue } else
                // { flush } }` hides the accumulator's loop definitions from
                // the flush): the other side names it.
                let named = match (count.as_slice(), flush.as_slice()) {
                    ([one], [two]) if one == two => Some(*one),
                    ([one], []) => Some(*one),
                    _ => None,
                };
                if let Some(one) = named {
                    let [count_name, _, flush_name] =
                        media_group_names(one).ok_or_else(|| refuse(index, format!("no group {one}")))?;
                    names[index] = Some(count_name);
                    names[index + 1] = Some(flush_name);
                    index += 2;
                    continue;
                }
            }
            let name = if field.is_plain(LeafKind::Number) {
                let mut boundaries = nearest_feature(&slices[index], MEDIA_BOUNDARIES);
                if boundaries.is_empty() {
                    // `probe(win, ok ? "max-resolution" : "-webkit-…", …)`:
                    // the query text is an arm of a conditional argument.
                    let arms = ValueSlice { arguments: conditional_arguments(ctx, field, 3), ..ValueSlice::default() };
                    boundaries = nearest_feature(&arms, MEDIA_BOUNDARIES);
                }
                match boundaries.as_slice() {
                    [one] => *one,
                    [] => match ctx_index_read(ctx, field) {
                        Some(0) => "aspectNumerator",
                        Some(1) => "aspectDenominator",
                        _ => return Err(refuse(index, "numeric field reads no boundary query or aspect index".into())),
                    },
                    many => return Err(refuse(index, format!("reads boundaries {many:?}"))),
                }
            } else if slices[index].arguments.contains_key("(q: 1)") && !field.is_array() {
                "fakeQueryCheck"
            } else {
                return Err(refuse(index, "not a group, boundary or (q: 1) byte".into()));
            };
            names[index] = Some(name);
            index += 1;
        }
        for (field, name) in fields.iter().zip(&names) {
            let name = name.unwrap_or_default();
            if binding.names().any(|bound| bound == name) {
                return Err(BindRefusal::new(format!("MatchMedia: message {message}: {name} is written by two fields")));
            }
            binding.insert(message, field.key.clone(), name);
        }
    }
    Ok(binding)
}

/// String literals passed as both arms of a conditional call argument
/// (`f(a, c ? "x" : "y")`) in the field's write-site slice, `hops`
/// definitions deep, with the smallest depth.
fn conditional_arguments(ctx: &FinderCtx, field: &Field, hops: usize) -> std::collections::BTreeMap<String, usize> {
    use oxc_ast::ast::{Argument, Expression};
    use oxc_ast::AstKind;
    let mut out = std::collections::BTreeMap::new();
    for leaf in field.leaves {
        if literal_only(leaf) {
            continue;
        }
        let Some(value) = ctx.caller_value_node(leaf) else { continue };
        let slice = ctx.slice_of(value, hops);
        let mut roots = vec![(value, 0usize)];
        roots.extend(
            slice
                .definitions
                .iter()
                .filter_map(|(_, definition, depth)| ctx.definition_value(*definition).map(|node| (node, *depth))),
        );
        for (root, depth) in roots {
            let span = oxc_span::GetSpan::span(&ctx.parse.nodes.kind(root));
            for id in ctx.nodes_within(span) {
                let AstKind::CallExpression(call) = ctx.parse.nodes.kind(id) else { continue };
                for argument in &call.arguments {
                    // `q = ok ? "a" : "b"` split into arms: `if (ok) { q = "a"; } else { q = "b"; } probe(win, q, …)`,
                    // a local whose every write is a string literal.
                    if let Argument::Identifier(identifier) = argument {
                        for text in literal_writes(ctx, identifier) {
                            let entry = out.entry(text).or_insert(depth);
                            *entry = (*entry).min(depth);
                        }
                        continue;
                    }
                    let Argument::ConditionalExpression(conditional) = argument else { continue };
                    let (Expression::StringLiteral(yes), Expression::StringLiteral(no)) =
                        (&conditional.consequent, &conditional.alternate)
                    else {
                        continue;
                    };
                    for text in [yes.value.as_str(), no.value.as_str()] {
                        let entry = out.entry(text.to_string()).or_insert(depth);
                        *entry = (*entry).min(depth);
                    }
                }
            }
        }
    }
    out
}

/// The string literals a local holds: every write of the binding assigns a
/// string literal (a declaration without initialiser writes nothing).
/// Empty when any write is something else.
fn literal_writes(ctx: &FinderCtx, identifier: &oxc_ast::ast::IdentifierReference) -> Vec<String> {
    use oxc_ast::ast::Expression;
    use oxc_ast::AstKind;
    let Some(symbol) = ctx.identifier_symbol(identifier) else { return Vec::new() };
    let scoping = ctx.parse.scoping;
    let nodes = ctx.parse.nodes;
    let declarations: Vec<_> = scoping.symbol_declarations(symbol).collect();
    let [declaration] = declarations.as_slice() else { return Vec::new() };
    if !matches!(nodes.kind(*declaration), AstKind::VariableDeclarator(declarator) if declarator.init.is_none()) {
        return Vec::new();
    }
    let mut out = Vec::new();
    for reference in scoping.get_resolved_references(symbol) {
        if !reference.is_write() {
            continue;
        }
        let target = nodes.parent_id(reference.node_id());
        let assignment = match nodes.kind(target) {
            AstKind::AssignmentExpression(_) => target,
            _ => nodes.parent_id(target),
        };
        match nodes.kind(assignment) {
            AstKind::AssignmentExpression(assignment)
                if assignment.operator == oxc_syntax::operator::AssignmentOperator::Assign =>
            {
                match &assignment.right {
                    Expression::StringLiteral(literal) => out.push(literal.value.to_string()),
                    _ => return Vec::new(),
                }
            }
            _ => return Vec::new(),
        }
    }
    out
}

/// The numeric index `r[0]` / `r[1]` the field's value reads, following
/// plain copies (`x = r[1]`) up to three definitions back.
fn ctx_index_read(ctx: &FinderCtx, field: &Field) -> Option<u32> {
    use oxc_ast::ast::Expression;
    use oxc_ast::AstKind;
    let leaf = field.leaves.first()?;
    let mut node = ctx.caller_value_node(leaf)?;
    for _ in 0..4 {
        match ctx.parse.nodes.kind(node) {
            AstKind::ComputedMemberExpression(member) => {
                return match &member.expression {
                    Expression::NumericLiteral(literal) if literal.value.fract() == 0.0 && literal.value >= 0.0 => {
                        Some(literal.value as u32)
                    }
                    _ => None,
                };
            }
            AstKind::IdentifierReference(_) => {
                let slice = ctx.slice_of(node, 1);
                let (_, definition, _) = slice.definitions.first()?;
                node = ctx.definition_value(*definition)?;
            }
            AstKind::ParenthesizedExpression(inner) => node = ctx.node_with_span(oxc_span::GetSpan::span(&inner.expression))?,
            _ => return None,
        }
    }
    None
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `MatchMedia`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "MatchMedia",
    summary: "Asks the browser a long list of CSS media queries with window.matchMedia. For yes/no style features (hover, pointer, color scheme, display mode, ...) it records which of the possible values match. For numeric features (viewport width and height, color depth, resolution, aspect ratio) it finds the boundary value by searching with max-* queries. It also checks a made-up query, (q: 1), that a real browser should never match. One message is sent during collection.",
    why: "Media-query answers describe the real device and user settings (screen, input devices, dark mode, reduced motion, colour gamut) and are hard to fake consistently; a vendor likely compares them with the claimed user agent and other screen values to spot spoofing, headless browsers and hooked matchMedia.",
    sample_payload: Some(r#"{
  "anyHoverBits": {
  },
  "anyHoverCount": 2,
  "anyHoverFlush": {
    "hover": true,
    "none": false
  },
  "anyPointerBits": {
  },
  "anyPointerCount": 3,
  "anyPointerFlush": {
    "coarse": false,
    "fine": true,
    "none": false
  },
  "aspectDenominator": 1125899906842624,
  "aspectNumerator": 7532152686891421,
  "colorGamutBits": {
  },
  "colorGamutCount": 3,
  "colorGamutFlush": {
    "p3": false,
    "rec2020": false,
    "srgb": true
  },
  "displayModeBits": {
  },
  "displayModeCount": 4,
  "displayModeFlush": {
    "browser": true,
    "fullscreen": false,
    "minimal-ui": false,
    "standalone": false
  },
  "fakeQueryCheck": false,
  "forcedColorsBits": {
  },
  "forcedColorsCount": 2,
  "forcedColorsFlush": {
    "active": false,
    "none": true
  },
  "gridBits": {
  },
  "gridCount": 2,
  "gridFlush": {
    "0": true,
    "1": false
  },
  "hoverBits": {
  },
  "hoverCount": 2,
  "hoverFlush": {
    "hover": true,
    "none": false
  },
  "invertedColorsBits": {
  },
  "invertedColorsCount": 2,
  "invertedColorsFlush": {
    "inverted": false,
    "none": false
  },
  "lightLevelBits": {
  },
  "lightLevelCount": 3,
  "lightLevelFlush": {
    "dim": false,
    "normal": false,
    "washed": false
  },
  "maxColor": 8,
  "maxColorIndex": 0,
  "maxHeight": 287,
  "maxMonochrome": 0,
  "maxWidth": 1920,
  "orientationBits": {
  },
  "orientationCount": 2,
  "orientationFlush": {
    "landscape": true,
    "portrait": false
  },
  "overflowBlockBits": {
  },
  "overflowBlockCount": 4,
  "overflowBlockFlush": {
    "none": false,
    "optional-paged": false,
    "paged": false,
    "scroll": true
  },
  "overflowInlineBits": {
  },
  "overflowInlineCount": 2,
  "overflowInlineFlush": {
    "none": false,
    "scroll": true
  },
  "pointerBits": {
  },
  "pointerCount": 3,
  "pointerFlush": {
    "coarse": false,
    "fine": true,
    "none": false
  },
  "prefersColorSchemeBits": {
  },
  "prefersColorSchemeCount": 3,
  "prefersColorSchemeFlush": {
    "dark": true,
    "light": false,
    "no-preference": false
  },
  "prefersContrastBits": {
  },
  "prefersContrastCount": 3,
  "prefersContrastFlush": {
    "high": false,
    "low": false,
    "no-preference": true
  },
  "prefersReducedMotionBits": {
  },
  "prefersReducedMotionCount": 2,
  "prefersReducedMotionFlush": {
    "no-preference": true,
    "reduce": false
  },
  "prefersReducedTransparencyBits": {
  },
  "prefersReducedTransparencyCount": 2,
  "prefersReducedTransparencyFlush": {
    "no-preference": true,
    "reduce": false
  },
  "resolution": 192,
  "scanBits": {
  },
  "scanCount": 2,
  "scanFlush": {
    "interlace": false,
    "progressive": false
  },
  "scriptingBits": {
  },
  "scriptingCount": 3,
  "scriptingFlush": {
    "enabled": true,
    "initial-only": false,
    "none": false
  },
  "updateBits": {
  },
  "updateCount": 3,
  "updateFlush": {
    "fast": true,
    "none": false,
    "slow": false
  }
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number("gridCount", "grid group: number of candidate values tested, always 2 (0, 1)."),
        CardField::byte(
            "gridBits",
            "grid group: in-loop flush byte of the match-bit loop, written once per 8 candidates; never written because the group has fewer than 8 candidates.",
        )
        .flags(&["0", "1"]),
        CardField::byte(
            "gridFlush",
            "grid group: the trailing flush byte holding the packed match bits, 1 = matches, one bit per candidate ((grid: 0), (grid: 1)); bit order (ascending or descending) varies per build.",
        )
        .flags(&["0", "1"]),
        CardField::number(
            "overflowInlineCount",
            "overflow-inline group: number of candidate values tested, always 2 (none, scroll).",
        ),
        CardField::byte(
            "overflowInlineBits",
            "overflow-inline group: in-loop flush byte of the match-bit loop, written once per 8 candidates; never written because the group has fewer than 8 candidates.",
        )
        .flags(&["none", "scroll"]),
        CardField::byte(
            "overflowInlineFlush",
            "overflow-inline group: the trailing flush byte holding the packed match bits, 1 = matches, one bit per candidate (none, scroll); bit order (ascending or descending) varies per build.",
        )
        .flags(&["none", "scroll"]),
        CardField::number(
            "resolution",
            "Screen resolution boundary: the smallest whole dpi value where (max-resolution: N dpi) matches (e.g. 96 or 192). If min-resolution is not supported, the boundary of -webkit-max-device-pixel-ratio instead (e.g. 1, 2 or 3).",
        ),
        CardField::number(
            "anyPointerCount",
            "any-pointer group: number of candidate values tested, always 3 (none, coarse, fine).",
        ),
        CardField::byte(
            "anyPointerBits",
            "any-pointer group: in-loop flush byte of the match-bit loop, written once per 8 candidates; never written because the group has fewer than 8 candidates.",
        )
        .flags(&["none", "coarse", "fine"]),
        CardField::byte(
            "anyPointerFlush",
            "any-pointer group: the trailing flush byte holding the packed match bits, 1 = matches, one bit per candidate (none, coarse, fine); bit order (ascending or descending) varies per build.",
        )
        .flags(&["none", "coarse", "fine"]),
        CardField::number(
            "maxMonochrome",
            "Bits per pixel of a monochrome display found with max-monochrome; 0 on normal colour screens.",
        ),
        CardField::number("maxHeight", "Viewport height in CSS pixels (rounded up), found with max-height."),
        CardField::number(
            "aspectNumerator",
            "Numerator of a fraction that approximates the viewport width/height ratio from below, found with max-aspect-ratio (e.g. 16 in 16/9).",
        ),
        CardField::number(
            "maxColor",
            "Bits per colour component found with max-color, typically 8 (10 on HDR/wide screens).",
        ),
        CardField::number(
            "prefersContrastCount",
            "prefers-contrast group: number of candidate values tested, always 3 (no-preference, high, low).",
        ),
        CardField::byte(
            "prefersContrastBits",
            "prefers-contrast group: in-loop flush byte of the match-bit loop, written once per 8 candidates; never written because the group has fewer than 8 candidates.",
        )
        .flags(&["no-preference", "high", "low"]),
        CardField::byte(
            "prefersContrastFlush",
            "prefers-contrast group: the trailing flush byte holding the packed match bits, 1 = matches, one bit per candidate (no-preference, high, low); bit order (ascending or descending) varies per build.",
        )
        .flags(&["no-preference", "high", "low"]),
        CardField::number(
            "aspectDenominator",
            "Denominator of the viewport aspect-ratio fraction (pairs with aspectNumerator).",
        ),
        CardField::number(
            "scriptingCount",
            "scripting group: number of candidate values tested, always 3 (none, initial-only, enabled).",
        ),
        CardField::byte(
            "scriptingBits",
            "scripting group: in-loop flush byte of the match-bit loop, written once per 8 candidates; never written because the group has fewer than 8 candidates.",
        )
        .flags(&["none", "initial-only", "enabled"]),
        CardField::byte(
            "scriptingFlush",
            "scripting group: the trailing flush byte holding the packed match bits, 1 = matches, one bit per candidate (none, initial-only, enabled); bit order (ascending or descending) varies per build.",
        )
        .flags(&["none", "initial-only", "enabled"]),
        CardField::number(
            "lightLevelCount",
            "light-level group: number of candidate values tested, always 3 (normal, dim, washed).",
        ),
        CardField::byte(
            "lightLevelBits",
            "light-level group: in-loop flush byte of the match-bit loop, written once per 8 candidates; never written because the group has fewer than 8 candidates.",
        )
        .flags(&["normal", "dim", "washed"]),
        CardField::byte(
            "lightLevelFlush",
            "light-level group: the trailing flush byte holding the packed match bits, 1 = matches, one bit per candidate (normal, dim, washed); bit order (ascending or descending) varies per build.",
        )
        .flags(&["normal", "dim", "washed"]),
        CardField::number(
            "displayModeCount",
            "display-mode group: number of candidate values tested, always 4 (fullscreen, standalone, minimal-ui, browser).",
        ),
        CardField::byte(
            "displayModeBits",
            "display-mode group: in-loop flush byte of the match-bit loop, written once per 8 candidates; never written because the group has fewer than 8 candidates.",
        )
        .flags(&["fullscreen", "standalone", "minimal-ui", "browser"]),
        CardField::byte(
            "displayModeFlush",
            "display-mode group: the trailing flush byte holding the packed match bits, 1 = matches, one bit per candidate (fullscreen, standalone, minimal-ui, browser); bit order (ascending or descending) varies per build.",
        )
        .flags(&["fullscreen", "standalone", "minimal-ui", "browser"]),
        CardField::number(
            "forcedColorsCount",
            "forced-colors group: number of candidate values tested, always 2 (none, active).",
        ),
        CardField::byte(
            "forcedColorsBits",
            "forced-colors group: in-loop flush byte of the match-bit loop, written once per 8 candidates; never written because the group has fewer than 8 candidates.",
        )
        .flags(&["none", "active"]),
        CardField::byte(
            "forcedColorsFlush",
            "forced-colors group: the trailing flush byte holding the packed match bits, 1 = matches, one bit per candidate (none, active); bit order (ascending or descending) varies per build.",
        )
        .flags(&["none", "active"]),
        CardField::number(
            "maxColorIndex",
            "Size of the colour lookup table found with max-color-index; 0 on normal screens.",
        ),
        CardField::number(
            "hoverCount",
            "hover group: number of candidate values tested, always 2 (none, hover).",
        ),
        CardField::byte(
            "hoverBits",
            "hover group: in-loop flush byte of the match-bit loop, written once per 8 candidates; never written because the group has fewer than 8 candidates.",
        )
        .flags(&["none", "hover"]),
        CardField::byte(
            "hoverFlush",
            "hover group: the trailing flush byte holding the packed match bits, 1 = matches, one bit per candidate (none, hover); bit order (ascending or descending) varies per build.",
        )
        .flags(&["none", "hover"]),
        CardField::number(
            "prefersColorSchemeCount",
            "prefers-color-scheme group: number of candidate values tested, always 3 (no-preference, light, dark).",
        ),
        CardField::byte(
            "prefersColorSchemeBits",
            "prefers-color-scheme group: in-loop flush byte of the match-bit loop, written once per 8 candidates; never written because the group has fewer than 8 candidates.",
        )
        .flags(&["no-preference", "light", "dark"]),
        CardField::byte(
            "prefersColorSchemeFlush",
            "prefers-color-scheme group: the trailing flush byte holding the packed match bits, 1 = matches, one bit per candidate (no-preference, light, dark); bit order (ascending or descending) varies per build.",
        )
        .flags(&["no-preference", "light", "dark"]),
        CardField::number(
            "colorGamutCount",
            "color-gamut group: number of candidate values tested, always 3 (srgb, p3, rec2020).",
        ),
        CardField::byte(
            "colorGamutBits",
            "color-gamut group: in-loop flush byte of the match-bit loop, written once per 8 candidates; never written because the group has fewer than 8 candidates.",
        )
        .flags(&["srgb", "p3", "rec2020"]),
        CardField::byte(
            "colorGamutFlush",
            "color-gamut group: the trailing flush byte holding the packed match bits, 1 = matches, one bit per candidate (srgb, p3, rec2020); bit order (ascending or descending) varies per build.",
        )
        .flags(&["srgb", "p3", "rec2020"]),
        CardField::number(
            "invertedColorsCount",
            "inverted-colors group: number of candidate values tested, always 2 (none, inverted).",
        ),
        CardField::byte(
            "invertedColorsBits",
            "inverted-colors group: in-loop flush byte of the match-bit loop, written once per 8 candidates; never written because the group has fewer than 8 candidates.",
        )
        .flags(&["none", "inverted"]),
        CardField::byte(
            "invertedColorsFlush",
            "inverted-colors group: the trailing flush byte holding the packed match bits, 1 = matches, one bit per candidate (none, inverted); bit order (ascending or descending) varies per build.",
        )
        .flags(&["none", "inverted"]),
        CardField::number(
            "orientationCount",
            "orientation group: number of candidate values tested, always 2 (landscape, portrait).",
        ),
        CardField::byte(
            "orientationBits",
            "orientation group: in-loop flush byte of the match-bit loop, written once per 8 candidates; never written because the group has fewer than 8 candidates.",
        )
        .flags(&["landscape", "portrait"]),
        CardField::byte(
            "orientationFlush",
            "orientation group: the trailing flush byte holding the packed match bits, 1 = matches, one bit per candidate (landscape, portrait); bit order (ascending or descending) varies per build.",
        )
        .flags(&["landscape", "portrait"]),
        CardField::number(
            "scanCount",
            "scan group: number of candidate values tested, always 2 (interlace, progressive).",
        ),
        CardField::byte(
            "scanBits",
            "scan group: in-loop flush byte of the match-bit loop, written once per 8 candidates; never written because the group has fewer than 8 candidates.",
        )
        .flags(&["interlace", "progressive"]),
        CardField::byte(
            "scanFlush",
            "scan group: the trailing flush byte holding the packed match bits, 1 = matches, one bit per candidate (interlace, progressive); bit order (ascending or descending) varies per build.",
        )
        .flags(&["interlace", "progressive"]),
        CardField::number(
            "prefersReducedTransparencyCount",
            "prefers-reduced-transparency group: number of candidate values tested, always 2 (no-preference, reduce).",
        ),
        CardField::byte(
            "prefersReducedTransparencyBits",
            "prefers-reduced-transparency group: in-loop flush byte of the match-bit loop, written once per 8 candidates; never written because the group has fewer than 8 candidates.",
        )
        .flags(&["no-preference", "reduce"]),
        CardField::byte(
            "prefersReducedTransparencyFlush",
            "prefers-reduced-transparency group: the trailing flush byte holding the packed match bits, 1 = matches, one bit per candidate (no-preference, reduce); bit order (ascending or descending) varies per build.",
        )
        .flags(&["no-preference", "reduce"]),
        CardField::boolean(
            "fakeQueryCheck",
            "Whether the made-up query (q: 1) matches: one fixed byte value for yes and another for no (the pair changes per build). A real browser always answers no; yes likely means matchMedia is hooked or emulated.",
        ),
        CardField::number(
            "updateCount",
            "update group: number of candidate values tested, always 3 (none, slow, fast).",
        ),
        CardField::byte(
            "updateBits",
            "update group: in-loop flush byte of the match-bit loop, written once per 8 candidates; never written because the group has fewer than 8 candidates.",
        )
        .flags(&["none", "slow", "fast"]),
        CardField::byte(
            "updateFlush",
            "update group: the trailing flush byte holding the packed match bits, 1 = matches, one bit per candidate (none, slow, fast); bit order (ascending or descending) varies per build.",
        )
        .flags(&["none", "slow", "fast"]),
        CardField::number(
            "pointerCount",
            "pointer group: number of candidate values tested, always 3 (none, coarse, fine).",
        ),
        CardField::byte(
            "pointerBits",
            "pointer group: in-loop flush byte of the match-bit loop, written once per 8 candidates; never written because the group has fewer than 8 candidates.",
        )
        .flags(&["none", "coarse", "fine"]),
        CardField::byte(
            "pointerFlush",
            "pointer group: the trailing flush byte holding the packed match bits, 1 = matches, one bit per candidate (none, coarse, fine); bit order (ascending or descending) varies per build.",
        )
        .flags(&["none", "coarse", "fine"]),
        CardField::number(
            "anyHoverCount",
            "any-hover group: number of candidate values tested, always 2 (none, hover).",
        ),
        CardField::byte(
            "anyHoverBits",
            "any-hover group: in-loop flush byte of the match-bit loop, written once per 8 candidates; never written because the group has fewer than 8 candidates.",
        )
        .flags(&["none", "hover"]),
        CardField::byte(
            "anyHoverFlush",
            "any-hover group: the trailing flush byte holding the packed match bits, 1 = matches, one bit per candidate (none, hover); bit order (ascending or descending) varies per build.",
        )
        .flags(&["none", "hover"]),
        CardField::number("maxWidth", "Viewport width in CSS pixels (rounded up), found with max-width."),
        CardField::number(
            "prefersReducedMotionCount",
            "prefers-reduced-motion group: number of candidate values tested, always 2 (no-preference, reduce).",
        ),
        CardField::byte(
            "prefersReducedMotionBits",
            "prefers-reduced-motion group: in-loop flush byte of the match-bit loop, written once per 8 candidates; never written because the group has fewer than 8 candidates.",
        )
        .flags(&["no-preference", "reduce"]),
        CardField::byte(
            "prefersReducedMotionFlush",
            "prefers-reduced-motion group: the trailing flush byte holding the packed match bits, 1 = matches, one bit per candidate (no-preference, reduce); bit order (ascending or descending) varies per build.",
        )
        .flags(&["no-preference", "reduce"]),
        CardField::number(
            "overflowBlockCount",
            "overflow-block group: number of candidate values tested, always 4 (none, scroll, optional-paged, paged).",
        ),
        CardField::byte(
            "overflowBlockBits",
            "overflow-block group: in-loop flush byte of the match-bit loop, written once per 8 candidates; never written because the group has fewer than 8 candidates.",
        )
        .flags(&["none", "scroll", "optional-paged", "paged"]),
        CardField::byte(
            "overflowBlockFlush",
            "overflow-block group: the trailing flush byte holding the packed match bits, 1 = matches, one bit per candidate (none, scroll, optional-paged, paged); bit order (ascending or descending) varies per build.",
        )
        .flags(&["none", "scroll", "optional-paged", "paged"]),
    ]),
};
