//! `WebGlContext`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, LayoutField, LayoutNode, LeafKind, LeafOp, LeafView, SignalLayout, SlotView};
use crate::signals::analysis::{layout_nodes::*, render_probes::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `WebGlContext`. Card: [`CARD`].
///
/// Shape: 20+ leaves with several strings. Evidence: `getParameter` and
/// `getSupportedExtensions` on the slot's path.
pub(crate) fn recognize_webgl_context(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    let claim = claim_for(view, ctx, "WebGlContext")?;
    // A shared producer that also runs the WebGL probe reaches its APIs
    // from any slot (through the anchor closure, or the value closure's
    // shared stream and buffers); the slot's own values must read report
    // properties: a navigator/screen census in the same producer reads none.
    {
        let reads = WEBGL_PARAMETERS
            .iter()
            .filter(|name| !matches!(**name, "version" | "vendor"))
            .filter(|name| {
                let property = format!(".{name}");
                view.leaves().any(|leaf| leaf.transforms.iter().any(|t| *t == property))
            })
            .count();
        if reads == 0 {
            return None;
        }
    }
    Some(claim)
}

/// Report properties that name a card field directly (`.alphaBits` of the
/// parameter map).
const WEBGL_PARAMETERS: &[&str] = &[
    "version",
    "renderer",
    "vendor",
    "shadingLanguageVersion",
    "alphaBits",
    "redBits",
    "greenBits",
    "blueBits",
    "depthBits",
    "stencilBits",
    "maxRenderbufferSize",
    "maxCubeMapTextureSize",
    "maxTextureSize",
    "maxTextureImageUnits",
    "maxVertexTextureImageUnits",
    "maxCombinedTextureImageUnits",
    "maxVertexUniformVectors",
    "maxFragmentUniformVectors",
    "maxVaryingVectors",
    "maxVertexAttribs",
    "contextProperties",
    "maxAnisotropy",
];

/// The card name of one top-level WebGL field, from the report property its
/// value leaves read: the first transform of a value leaf is the property and
/// the second its container (`.params`, `.debugInfo`, `.dimensions`, …);
/// later transforms are shared-temporary noise and are not read. The packed
/// flags byte is the byte leaf built from `.antialias` /
/// `.collectedUsingWebWorker`.
fn webgl_field_name(ctx: &FinderCtx, leaves: &[&LeafView]) -> Result<Option<&'static str>, String> {
    let mut names = std::collections::BTreeSet::new();
    for leaf in leaves {
        let flags = leaf.op == LeafOp::Byte
            && leaf
                .transforms
                .iter()
                .any(|t| t == ".antialias" || t == ".collectedUsingWebWorker");
        let first = leaf.transforms.first().map(String::as_str);
        let second = leaf.transforms.get(1).map(String::as_str);
        // `pair = [write, params]; pair[0](pair[1].renderer, …)` (the
        // tracer's method pair): the container is the object the pair
        // element holds, past the element read and the pair literal.
        // A reassigned container register (`x = report.parameters; …;
        // x = x.params; x.renderer`) lists the outer report objects
        // (`.parameters`, `.data`) before the map it now holds: the
        // container is the first of the two pair owners in the chain.
        let container = match first {
            Some(".vendor" | ".renderer") => {
                let mut rest = leaf.transforms.iter().skip(1).map(String::as_str);
                let owner = rest.clone().find(|t| matches!(*t, ".params" | ".debugInfo"));
                owner.or_else(|| rest.find(|t| !matches!(*t, "[index]" | "array")))
            }
            _ => second,
        };
        let name = match (first, container) {
            _ if flags => Some("flags"),
            // The masked pair lives in the parameter map; the debug-info pair
            // is its own object (`.debugInfo`, or built on the page path).
            (Some(".vendor"), Some(".params")) => Some("vendor"),
            (Some(".renderer"), Some(".params")) => Some("renderer"),
            (Some(".vendor"), Some(".debugInfo")) => Some("unmaskedVendor"),
            (Some(".renderer"), Some(".debugInfo")) => Some("unmaskedRenderer"),
            // The transforms lost the owner (`pair = [write, x]; pair[0](pair[1].vendor, …)`
            // through a register the guards reuse): the member path of the
            // value names it; otherwise the debug-info pair.
            (Some(".vendor" | ".renderer"), _) => {
                use crate::signals::analysis::write_sites::PathStep;
                let path = leaf_path(ctx, leaf);
                let params = matches!(path.as_slice(), [.., PathStep::Property(owner), PathStep::Property(_)] if owner == "params");
                Some(match (first, params) {
                    (Some(".vendor"), true) => "vendor",
                    (Some(".vendor"), false) => "unmaskedVendor",
                    (_, true) => "renderer",
                    (_, false) => "unmaskedRenderer",
                })
            }
            (Some(".length" | "[index]"), Some(".dimensions")) => Some("dimensions"),
            (Some(".length" | "[index]"), Some(".shaderPrecisions")) => Some("shaderPrecisions"),
            (Some(".length" | "[index]"), Some(".supportedExtensions")) => Some("supportedExtensions"),
            (Some(property), _) => {
                let property = property.strip_prefix('.').unwrap_or("");
                WEBGL_PARAMETERS.iter().copied().find(|name| *name == property)
            }
            _ => None,
        };
        if let Some(name) = name {
            names.insert(name);
        }
    }
    match names.len() {
        0 => Ok(None),
        1 => Ok(names.into_iter().next()),
        _ => Err(format!("leaves read several report properties {names:?}")),
    }
}

/// The card name of a field whose value leaves lost their report property in
/// the transforms (the container was reassigned, `x = x.dimensions`): the
/// member path of each value leaf, resolved through local definitions, reads
/// a list group's `.length` / `[i]` or a report property. Byte leaves (null
/// markers) are not read. `Ok(None)` when no value leaf has such a path.
fn webgl_path_name(ctx: &FinderCtx, leaves: &[&LeafView]) -> Result<Option<&'static str>, String> {
    use crate::signals::analysis::write_sites::PathStep;
    let mut names = std::collections::BTreeSet::new();
    for leaf in leaves.iter().filter(|leaf| leaf.op != LeafOp::Byte) {
        let path = leaf_path(ctx, leaf);
        let property = |step: Option<&PathStep>| match step {
            Some(PathStep::Property(name)) => Some(name.clone()),
            _ => None,
        };
        let name = match path.as_slice() {
            [.., container, PathStep::Property(last)] if last == "length" => {
                property(Some(container)).and_then(|group| webgl_list_names(&group).map(|(count, _)| count))
            }
            [.., container, PathStep::Index(_) | PathStep::Dynamic] => {
                property(Some(container)).and_then(|group| webgl_list_names(&group).map(|(count, _)| count))
            }
            [.., PathStep::Property(last)] => WEBGL_PARAMETERS.iter().copied().find(|name| *name == last),
            _ => None,
        };
        match name {
            Some(name) => {
                names.insert(name);
            }
            None => return Ok(None),
        }
    }
    match names.into_iter().collect::<Vec<_>>().as_slice() {
        [] => Ok(None),
        [name] => Ok(Some(name)),
        names => Err(format!("value leaves read several report properties {names:?}")),
    }
}

/// The count and item names of the three list groups.
fn webgl_list_names(group: &str) -> Option<(&'static str, &'static str)> {
    match group {
        "dimensions" => Some(("dimensions", "dimension")),
        "shaderPrecisions" => Some(("shaderPrecisions", "shaderPrecision")),
        "supportedExtensions" => Some(("supportedExtensions", "extension")),
        _ => None,
    }
}

/// Name the `count, list` record of a list group wherever it sits in the
/// field's choice tree; other records (literal null-marker pairs) are left
/// for the caller to report.
fn bind_webgl_list(node: &LayoutNode, message: usize, group: &str, binding: &mut FieldBinding) {
    match node {
        LayoutNode::Record { fields } => {
            let Some((count, item)) = webgl_list_names(group) else { return };
            if let [first, second] = fields.as_slice() {
                if is_scalar(&first.node) && is_array(&second.node) {
                    binding.insert(message, first.key.clone(), count);
                    binding.insert(message, second.key.clone(), item);
                }
            }
        }
        LayoutNode::Choice { arms } => arms
            .iter()
            .for_each(|arm| bind_webgl_list(&arm.node, message, group, binding)),
        _ => {}
    }
}

/// Record keys under `node` that `binding` leaves unnamed.
fn unbound_record_keys(node: &LayoutNode, message: usize, binding: &FieldBinding, out: &mut Vec<String>) {
    match node {
        LayoutNode::Record { fields } => {
            for field in fields {
                if binding.get(message, &field.key).is_none() {
                    out.push(field.key.clone());
                }
                unbound_record_keys(&field.node, message, binding, out);
            }
        }
        LayoutNode::Choice { arms } => arms
            .iter()
            .for_each(|arm| unbound_record_keys(&arm.node, message, binding, out)),
        LayoutNode::Array { item } => unbound_record_keys(item, message, binding, out),
        _ => {}
    }
}

/// The card name of a field that writes only null markers: the whole-group
/// marker of the group whose presence its innermost keyed guard tests
/// (`hasOwn(x, "params")`, …).
fn webgl_group_marker(ctx: &FinderCtx, leaves: &[&LeafView]) -> Result<&'static str, String> {
    let mut names = std::collections::BTreeSet::new();
    for leaf in leaves {
        if leaf.op != LeafOp::Byte {
            return Err("marker-only field writes a non-byte value".into());
        }
        let guards = ctx.guard_strings(&leaf.write);
        let Some(keys) = guards.iter().rev().find(|keys| !keys.is_empty()) else {
            return Err("marker has no keyed guard".into());
        };
        let name = match keys.as_slice() {
            [key] if key == "parameters" => "paramsMissing",
            [key] if key == "params" => "getParameterMissing",
            [key] if key == "debugInfo" => "debugInfoMissing",
            [key] if key == "antialias" => "antialiasMissing",
            _ => return Err(format!("marker guard tests {keys:?}, not a whole group")),
        };
        names.insert(name);
    }
    match names.into_iter().collect::<Vec<_>>().as_slice() {
        [name] => Ok(name),
        names => Err(format!("marker leaves name {names:?}")),
    }
}

/// Name the two-byte null markers (`tag, 1`: the number writer's
/// continuation form of tags 160–166) under `node`'s choice arms.
fn bind_marker_pairs(node: &LayoutNode, message: usize, binding: &mut FieldBinding) {
    match node {
        LayoutNode::Record { fields } => {
            let byte = |field: &LayoutField| match &field.node {
                LayoutNode::Leaf(leaf) if leaf.kind == LeafKind::Byte => leaf.byte,
                _ => None,
            };
            if let [first, second] = fields.as_slice() {
                if let (Some(tag), Some(high)) = (byte(first), byte(second)) {
                    if (160..192).contains(&tag) && (1..32).contains(&high) {
                        binding.insert(message, first.key.clone(), "nullMarker");
                        binding.insert(message, second.key.clone(), "nullFlag");
                    }
                }
            }
        }
        LayoutNode::Choice { arms } => arms.iter().for_each(|arm| bind_marker_pairs(&arm.node, message, binding)),
        _ => {}
    }
}

/// Every top-level field is named by the report property its value reads;
/// a field writing only null markers is the whole-group marker of the group
/// its keyed guard tests (`paramsMissing`, `getParameterMissing`,
/// `debugInfoMissing`, `antialiasMissing`); two-byte null markers inside a
/// field are `nullMarker, nullFlag`. Refuses on any other unnamed
/// field or record.
pub(crate) fn bind_webgl_context(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    const SIGNAL: &str = "WebGlContext";
    let mut binding = FieldBinding::new();
    for (message, layout_message) in layout.messages.iter().enumerate() {
        let leaves = field_leaves(view, layout, message).ok_or_else(|| refuse(SIGNAL, "leaf alignment failed"))?;
        let mut seen = std::collections::BTreeSet::new();
        for (field, leaves) in layout_message.fields.iter().zip(&leaves) {
            let name = webgl_field_name(ctx, leaves).map_err(|reason| refuse(SIGNAL, format!("{}: {reason}", field.key)))?;
            // The packed flags byte is the record's only unguarded computed
            // byte; its provenance can miss `.antialias` when the main-thread
            // printer routes it through a shared register.
            let computed_byte = matches!(&field.node, LayoutNode::Leaf(leaf) if leaf.kind == LeafKind::Byte && leaf.byte.is_none())
                && leaves.iter().all(|leaf| leaf.choice_depth == 0);
            let name = match name {
                Some(name) => Some(name),
                None => webgl_path_name(ctx, leaves).map_err(|reason| refuse(SIGNAL, format!("{}: {reason}", field.key)))?,
            };
            let name = match name {
                Some(name) => name,
                None if computed_byte => "flags",
                None => webgl_group_marker(ctx, leaves).map_err(|reason| refuse(SIGNAL, format!("{}: {reason}", field.key)))?,
            };
            if !seen.insert(name) {
                return Err(refuse(SIGNAL, format!("{}: {name} bound twice", field.key)));
            }
            binding.insert(message, field.key.clone(), name);
            if webgl_list_names(name).is_some() {
                bind_webgl_list(&field.node, message, name, &mut binding);
            }
            bind_marker_pairs(&field.node, message, &mut binding);
        }
        let mut unbound = Vec::new();
        for field in &layout_message.fields {
            unbound_record_keys(&field.node, message, &binding, &mut unbound);
        }
        if !unbound.is_empty() {
            return Err(refuse(
                SIGNAL,
                format!("message {message}: records {unbound:?} are neither a list nor a two-byte null marker"),
            ));
        }
    }
    Ok(binding)
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `WebGlContext`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "WebGlContext",
    summary: "Creates a WebGL context (in an OffscreenCanvas worker when possible, otherwise on a page canvas) and reads its static capabilities: 20 getParameter values, the unmasked GPU vendor and renderer, antialiasing, max anisotropy, line/point/viewport ranges, 12 shader precision formats, the extension list and a hash of the context's property names. It never draws. The worker path sends up to 8 progressively filled records as replies arrive. A missing value is replaced by a null marker; if a whole group (all parameters, the getParameter set, or the debug-info pair) is missing, one null marker replaces the whole group.",
    why: "The GPU renderer string and capability limits are among the strongest hardware fingerprints, and mismatches between them (or software renderers like SwiftShader) likely reveal virtual machines, headless browsers and spoofed GPUs.",
    sample_payload: Some(r#"{
  "alphaBits": 8,
  "blueBits": 8,
  "contextProperties": 747546536,
  "depthBits": 24,
  "dimensions": [
    "11",
    "1511",
    "1638416384"
  ],
  "flags": {
    "antialias": true,
    "collectedUsingWebWorker": true
  },
  "greenBits": 8,
  "maxAnisotropy": 16,
  "maxCombinedTextureImageUnits": 32,
  "maxCubeMapTextureSize": 16384,
  "maxFragmentUniformVectors": 1024,
  "maxRenderbufferSize": 16384,
  "maxTextureImageUnits": 16,
  "maxTextureSize": 16384,
  "maxVaryingVectors": 30,
  "maxVertexAttribs": 16,
  "maxVertexTextureImageUnits": 16,
  "maxVertexUniformVectors": 1024,
  "redBits": 8,
  "renderer": "WebKit WebGL",
  "shaderPrecisions": [
    "23127127",
    "23127127",
    "23127127",
    "23127127",
    "23127127",
    "23127127",
    "03130",
    "03130",
    "03130",
    "03130",
    "03130",
    "03130"
  ],
  "shadingLanguageVersion": "WebGL GLSL ES 1.0 (OpenGL ES GLSL ES 1.0 Chromium)",
  "stencilBits": 0,
  "supportedExtensions": [
    "ANGLE_instanced_arrays",
    "EXT_blend_minmax",
    "EXT_clip_control",
    "EXT_color_buffer_half_float",
    "EXT_depth_clamp",
    "EXT_disjoint_timer_query",
    "EXT_float_blend",
    "EXT_frag_depth",
    "EXT_polygon_offset_clamp",
    "EXT_shader_texture_lod",
    "EXT_texture_compression_bptc",
    "EXT_texture_compression_rgtc",
    "EXT_texture_filter_anisotropic",
    "EXT_texture_mirror_clamp_to_edge",
    "EXT_sRGB",
    "KHR_parallel_shader_compile",
    "OES_element_index_uint",
    "OES_fbo_render_mipmap",
    "OES_standard_derivatives",
    "OES_texture_float",
    "OES_texture_float_linear",
    "OES_texture_half_float",
    "OES_texture_half_float_linear",
    "OES_vertex_array_object",
    "WEBGL_blend_func_extended",
    "WEBGL_color_buffer_float",
    "WEBGL_compressed_texture_astc",
    "WEBGL_compressed_texture_etc",
    "WEBGL_compressed_texture_etc1",
    "WEBGL_compressed_texture_pvrtc",
    "WEBGL_compressed_texture_s3tc",
    "WEBGL_compressed_texture_s3tc_srgb",
    "WEBGL_debug_renderer_info",
    "WEBGL_debug_shaders",
    "WEBGL_depth_texture",
    "WEBGL_draw_buffers",
    "WEBGL_lose_context",
    "WEBGL_multi_draw",
    "WEBGL_polygon_mode"
  ],
  "unmaskedRenderer": "ANGLE (Apple, ANGLE Metal Renderer: Apple M5 Pro, Unspecified Version)",
  "unmaskedVendor": "Google Inc. (Apple)",
  "vendor": "WebKit",
  "version": "WebGL 1.0 (OpenGL ES 2.0 Chromium)"
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::null(
            "paramsMissing",
            "Null marker sent in place of every parameter field when the whole parameter object is missing; nothing is sent here when it is present.",
        ).nullable_pair(),
        CardField::null(
            "getParameterMissing",
            "Null marker sent in place of the getParameter values (version, renderer, vendor, bit depths and limits) when that set is missing or null; nothing is sent here when it is present.",
        ).nullable_pair(),
        CardField::null(
            "debugInfoMissing",
            "Null marker sent in place of unmaskedVendor and unmaskedRenderer when the debug-info pair is missing or null; nothing is sent here when it is present.",
        ).nullable_pair(),
        CardField::string(
            "version",
            "getParameter(VERSION), e.g. \"WebGL 1.0 (OpenGL ES 2.0 Chromium)\". Null marker if missing.",
        ).nullable_pair(),
        CardField::string(
            "renderer",
            "getParameter(RENDERER) (masked renderer, usually \"WebKit WebGL\"). Null marker if missing.",
        ).nullable_pair(),
        CardField::number("alphaBits", "getParameter(ALPHA_BITS), usually 8. Null marker if missing.").nullable_pair(),
        CardField::number("greenBits", "getParameter(GREEN_BITS), usually 8. Null marker if missing.").nullable_pair(),
        CardField::number(
            "maxRenderbufferSize",
            "getParameter(MAX_RENDERBUFFER_SIZE), e.g. 16384. Null marker if missing.",
        ).nullable_pair(),
        CardField::number(
            "maxCubeMapTextureSize",
            "getParameter(MAX_CUBE_MAP_TEXTURE_SIZE), e.g. 16384. Null marker if missing.",
        ).nullable_pair(),
        CardField::number(
            "stencilBits",
            "getParameter(STENCIL_BITS), usually 0 or 8. Null marker if missing.",
        ).nullable_pair(),
        CardField::string(
            "shadingLanguageVersion",
            "getParameter(SHADING_LANGUAGE_VERSION), e.g. \"WebGL GLSL ES 1.0 (...)\". Null marker if missing.",
        ).nullable_pair(),
        CardField::number(
            "maxTextureSize",
            "getParameter(MAX_TEXTURE_SIZE), e.g. 16384. Null marker if missing.",
        ).nullable_pair(),
        CardField::number(
            "contextProperties",
            "32-bit hash of the WebGL context object's property names joined with '|'; -1 placeholder before the worker reports it. Null marker if missing.",
        ).nullable_pair(),
        CardField::string(
            "unmaskedRenderer",
            "WEBGL_debug_renderer_info UNMASKED_RENDERER, the real GPU string, e.g. \"ANGLE (NVIDIA, ...)\"; null marker if the extension is unavailable.",
        ).nullable_pair(),
        CardField::number(
            "supportedExtensions",
            "Number of supported WebGL extensions that follow, or a null marker if missing.",
        ).nullable_pair(),
        CardField::string("extension", "One extension name, e.g. \"OES_texture_float\".")
            .repeat("once per supported extension"),
        CardField::string(
            "vendor",
            "getParameter(VENDOR) (masked vendor, usually \"WebKit\"). Null marker if missing.",
        ).nullable_pair(),
        CardField::number(
            "maxVertexUniformVectors",
            "getParameter(MAX_VERTEX_UNIFORM_VECTORS), e.g. 4096. Null marker if missing.",
        ).nullable_pair(),
        CardField::null(
            "antialiasMissing",
            "Null marker sent when the antialias attribute is missing or null (the flags byte then has antialias 0); nothing is sent here when it is present.",
        ).nullable_pair(),
        CardField::byte(
            "flags",
            "Packed flags: antialias (context was created with antialiasing), collectedUsingWebWorker (values came from an OffscreenCanvas worker rather than the page).",
        )
        .flags(&["antialias", "collectedUsingWebWorker"]),
        CardField::number(
            "dimensions",
            "Number of range strings that follow (3), or a null marker if missing.",
        ).nullable_pair(),
        CardField::string(
            "dimension",
            "ALIASED_LINE_WIDTH_RANGE, ALIASED_POINT_SIZE_RANGE and MAX_VIEWPORT_DIMS, each array joined with no separator (e.g. \"11\", \"11024\", \"3276832768\"); empty string if not an array.",
        )
        .repeat("once per range (3 entries)"),
        CardField::number(
            "maxAnisotropy",
            "MAX_TEXTURE_MAX_ANISOTROPY_EXT (e.g. 16), -1 if no anisotropic-filtering extension (NaN on the worker path in that case), or a null marker if missing.",
        ).nullable_pair(),
        CardField::number(
            "maxTextureImageUnits",
            "getParameter(MAX_TEXTURE_IMAGE_UNITS), e.g. 16. Null marker if missing.",
        ).nullable_pair(),
        CardField::string(
            "unmaskedVendor",
            "WEBGL_debug_renderer_info UNMASKED_VENDOR, e.g. \"Google Inc. (NVIDIA)\"; null marker if unavailable.",
        ).nullable_pair(),
        CardField::number("depthBits", "getParameter(DEPTH_BITS), usually 24. Null marker if missing.").nullable_pair(),
        CardField::number("blueBits", "getParameter(BLUE_BITS), usually 8. Null marker if missing.").nullable_pair(),
        CardField::number(
            "maxVaryingVectors",
            "getParameter(MAX_VARYING_VECTORS), e.g. 30. Null marker if missing.",
        ).nullable_pair(),
        CardField::number(
            "maxVertexTextureImageUnits",
            "getParameter(MAX_VERTEX_TEXTURE_IMAGE_UNITS), e.g. 16. Null marker if missing.",
        ).nullable_pair(),
        CardField::number("redBits", "getParameter(RED_BITS), usually 8. Null marker if missing.").nullable_pair(),
        CardField::number(
            "shaderPrecisions",
            "Number of shader precision strings that follow (12), or a null marker if missing or getShaderPrecisionFormat is unavailable.",
        ).nullable_pair(),
        CardField::string(
            "shaderPrecision",
            "precision + rangeMin + rangeMax concatenated (e.g. \"23127127\"), empty if null. The 12 entries are vertex then fragment shader, each for HIGH/MEDIUM/LOW_FLOAT, then vertex then fragment for HIGH/MEDIUM/LOW_INT.",
        )
        .repeat("once per shader/precision pair (12 entries)"),
        CardField::number(
            "maxVertexAttribs",
            "getParameter(MAX_VERTEX_ATTRIBS), e.g. 16. Null marker if missing.",
        ).nullable_pair(),
        CardField::number(
            "maxFragmentUniformVectors",
            "getParameter(MAX_FRAGMENT_UNIFORM_VECTORS), e.g. 1024. Null marker if missing.",
        ).nullable_pair(),
        CardField::number(
            "maxCombinedTextureImageUnits",
            "getParameter(MAX_COMBINED_TEXTURE_IMAGE_UNITS), e.g. 32. Null marker if missing.",
        ).nullable_pair(),
        CardField::null(
            "nullMarker",
            "First byte of a two-byte null marker, sent in place of a missing value for the later fields of the record (marker numbers 32 and up).",
        ),
        CardField::null(
            "nullFlag",
            "Second byte of a two-byte null marker (the high part of the marker number, 1 for these fields).",
        ),
    ]),
};
