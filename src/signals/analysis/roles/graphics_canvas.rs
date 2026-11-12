//! Package `p13_graphics_canvas`: canvas, WebGL, font-metric and media-codec
//! fingerprint probes.
//!
//! # What proves a role
//!
//! Every anchor is a stable host name or a stable call shape, never a generated
//! spelling:
//!
//! * a canvas probe is a function whose own body creates a canvas and samples
//!   it through `getContext("2d", { willReadFrequently: true })`,
//!   `fillText`/`fillRect`/`getImageData`, `toDataURL`, `drawImage` or
//!   `readPixels`;
//! * the WebGL group is the function that gets a `webgl`/`webgl2` context,
//!   compiles shaders and reads pixels; the font-metric group measures
//!   `measureText`; the seeded font-metric group forks a `families.map(...)`
//!   observable and subscribes an observer whose `next` member is the digest
//!   emitter; the media-codec group asks a `<video>` for `canPlayType` over the
//!   codec table;
//! * a probe closure is the function handed to the collector registrar with the
//!   grouping flag (`registrar(closure, false)` / `registrar(closure, true)`);
//!   its runner, `onEmpty` and `onError` callbacks are proven by the queue they
//!   are registered on, not by their position;
//! * a MAIN data binding is proven by its source-closed literal shape *and* by
//!   the proven function that consumes it: the glyph sample table is the array
//!   of `[glyph, [[x, y], ...]]` rows handed to the glyph observation queue, the
//!   emoji probe table is the array of `[glyph, x, y, channel]` rows handed to
//!   the emoji queue, the code-point/family tables are read by the font-metric
//!   probe, and the codec table is the string list a `<video>` asks
//!   `canPlayType` about.
//!
//! Both tracer shapes are read: named formals and `arguments[N]` are one slot
//! (through [`ParameterSlots`]), `x.m(...)` and `x["m"](...)` are one call, and
//! a callback may be an inline arrow or a hoisted declaration.
//!
//! Shared helpers named by earlier packages are not re-claimed: the keystream,
//! hash dispatch and task-queue roles stay with p05/p02/p03. This package owns
//! only their single MAIN aliases that no earlier package named, and proves
//! those aliases by the canvas probe that calls them.
//!
//! A role that matches more than one function or binding is dropped whole in
//! [`Out::into_unique`]: two equal candidates are a refusal, not a choice.

use std::collections::{BTreeMap, BTreeSet};

use oxc_ast::ast::*;
use oxc_ast::AstKind;
use oxc_semantic::{AstNodes, NodeId, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::operator::AssignmentOperator;

use crate::signals::extract::bindings::{
    numeric_index, object_members_of, property_key_name, ParameterSlots,
};
use crate::signals::schema::Confidence;
use super::{FinderClaim, FinderCtx, FinderRole, FinderRoleKind};
use crate::analysis;

// ---------------------------------------------------------------------------
// Roles
// ---------------------------------------------------------------------------

const EMOJI_CANVAS_COLLECTOR: &str = "emoji_canvas_pixel_collector";
const EMOJI_CANVAS_ALIAS: &str = "emoji_canvas_pixel_collector_alias";
const EMOJI_CANVAS_SAMPLER: &str = "emoji_canvas_glyph_sampler";
const EMOJI_CANVAS_EMITTER: &str = "emoji_canvas_sample_emitter";
const EMOJI_CANVAS_TEARDOWN: &str = "canvas_collect_task_teardown";
const WEBGL_PROBE: &str = "webgl_fingerprint_probe";
const WEBGL_PIXEL_HASH_EMITTER: &str = "webgl_pixel_hash_emitter";
const WEBGL_PIXEL_HASH_ERROR: &str = "webgl_pixel_hash_error_forwarder";
const FONT_METRICS_PROBE: &str = "font_metrics_probe";
const FONT_METRIC_WIDTH_RUNNER: &str = "font_metric_width_runner";
const FONT_METRICS_QUEUE_ERROR: &str = "font_metrics_queue_error";
const FONT_METRICS_QUEUE_EMPTY: &str = "font_metrics_queue_empty";
const SEEDED_FONT_METRICS_PROBE: &str = "seeded_font_metrics_probe";
const FONT_FAMILY_METRICS: &str = "font_family_metrics";
const FONT_METRIC_REDUCER: &str = "font_metric_hex_reducer";
const FONT_METRIC_HASH_MAPPER: &str = "font_metric_hash_mapper";
const SEEDED_FONT_METRICS_DIGEST: &str = "seeded_font_metrics_digest";
const FONT_PROBE_ERROR: &str = "font_probe_error_forwarder";
const MEDIA_CODEC_PROBE: &str = "media_codec_probe";
const MEDIA_CODEC_PRIMARY_HASH: &str = "media_codec_primary_hash";
const MEDIA_CODEC_PRIMARY_ERROR: &str = "media_codec_primary_error";
const MEDIA_CODEC_FALLBACK_HASH: &str = "media_codec_fallback_hash";
const MEDIA_CODEC_FALLBACK_ERROR: &str = "media_codec_fallback_error";
const WEBRTC_CANDIDATES: &str = "webrtc_candidate_emitter";
const WEBRTC_CANDIDATE_ERROR: &str = "webrtc_candidate_error";
const CANVAS_FEATURE_COLLECTOR: &str = "canvas_feature_collector";
const CANVAS_PROBE_CLOSURE: &str = "canvas_fingerprint_signals";
const IMAGE_PROBE_STARTER: &str = "canvas_pixel_probe_starter";
const IMAGE_LOAD_HANDLER: &str = "canvas_probe_image_load_handler";
const PIXEL_VECTOR_RECORDER: &str = "pixel_vector_sample_recorder";
const PIXEL_VECTOR_UNIQUE: &str = "pixel_vector_sample_acceptance";
const CANVAS_BLACK_SAMPLE: &str = "black_canvas_pixel_sample";
const CANVAS_DATA_URL_SAMPLE: &str = "canvas_data_url_sample";
const CANVAS_PATH_SAMPLE: &str = "canvas_path_fill_sample";
const CANVAS_SEEDED_SAMPLE: &str = "seeded_canvas_fill_sample";
const CANVAS_TEXT_SAMPLE: &str = "canvas_text_shadow_sample";
const CANVAS_QUEUE_ERROR: &str = "canvas_queue_error_forwarder";
const CANVAS_QUEUE_DIGEST: &str = "canvas_pixel_record_digest";
const BROWSER_FINGERPRINT_COLLECTOR: &str = "browser_fingerprint_collector";
const BROWSER_FINGERPRINT_CLOSURE: &str = "browser_fingerprint_signals";
const EMOJI_CHANNEL_SAMPLER: &str = "emoji_glyph_channel_sampler";
const EMOJI_RENDER_PROBE: &str = "emoji_render_probe";
const GLYPH_RENDER_PROBE: &str = "glyph_render_probe";
const BROWSER_PROBE_CLEANUP: &str = "browser_fingerprint_probe_cleanup";

const FORK_JOIN_ALIAS: &str = "fork_join_observable_factory_alias";
const REDUCE_OPERATOR_ALIAS: &str = "reduce_operator_alias";
const ASPECT_RATIO_ALIAS: &str = "aspect_ratio_prober_alias";
const CANVAS_GLYPH_SAMPLES: &str = "canvas_glyph_sample_table";
const EMOJI_RENDER_PROBES: &str = "emoji_render_probe_table";
const NATIVE_CODE_CHECK_TARGETS: &str = "native_code_check_targets";
const HASH_OBSERVABLE_BINDER: &str = "hash_observable_binder";
const FONT_PROBE_CODE_POINTS: &str = "font_probe_code_points";
const FONT_PROBE_FAMILIES: &str = "font_probe_families";
const STRING_FROM_CHAR_CODE: &str = "string_from_char_code";
const MEDIA_CODEC_PROBE_STRINGS: &str = "media_codec_probe_strings";
const SLICE_ARRAY_LIKE: &str = "slice_array_like";

const fn function(role: &'static str, name: &'static str) -> FinderRole {
    FinderRole { role, name, kind: FinderRoleKind::Function }
}

const fn alias(role: &'static str, name: &'static str) -> FinderRole {
    FinderRole { role, name, kind: FinderRoleKind::FunctionAlias }
}

const fn main(role: &'static str, name: &'static str) -> FinderRole {
    FinderRole { role, name, kind: FinderRoleKind::MainBinding }
}

/// Every role this package can claim, with the one name it receives.
///
/// The five MAIN aliases named by earlier packages are listed in the coverage
/// report, not here: the keystream byte factory, the async hash dispatcher, the
/// time-budgeted task queue factory and the two sequential queue aliases belong
/// to p05/p02/p03. Claiming them here would be a role conflict.
pub(crate) const VOCABULARY: &[FinderRole] = &[
    function(EMOJI_CANVAS_COLLECTOR, "createEmojiCanvasPixelCollector"),
    alias(EMOJI_CANVAS_ALIAS, "canvasPixelCollectorFactory"),
    function(EMOJI_CANVAS_SAMPLER, "sampleEmojiCanvasGlyphPixels"),
    function(EMOJI_CANVAS_EMITTER, "emitEmojiCanvasPixelSamples"),
    function(EMOJI_CANVAS_TEARDOWN, "teardownCanvasCollectTask"),
    function(WEBGL_PROBE, "emitWebglFingerprintProbe"),
    function(WEBGL_PIXEL_HASH_EMITTER, "emitWebglPixelHashProbe"),
    function(WEBGL_PIXEL_HASH_ERROR, "forwardWebglPixelHashError"),
    function(FONT_METRICS_PROBE, "emitFontMetricsProbe"),
    function(FONT_METRIC_WIDTH_RUNNER, "probeFontMetricWidths"),
    function(FONT_METRICS_QUEUE_ERROR, "throwFontProbeQueueError"),
    function(FONT_METRICS_QUEUE_EMPTY, "completeFontMetricsProbe"),
    function(SEEDED_FONT_METRICS_PROBE, "emitSeededFontMetricsProbe"),
    function(FONT_FAMILY_METRICS, "collectFontFamilyMetrics"),
    function(FONT_METRIC_REDUCER, "accumulateFontMetricHex"),
    function(FONT_METRIC_HASH_MAPPER, "hashFontMetricsText"),
    function(SEEDED_FONT_METRICS_DIGEST, "emitSeededFontMetricsDigest"),
    function(FONT_PROBE_ERROR, "forwardCanvasProbeError"),
    function(MEDIA_CODEC_PROBE, "emitMediaCodecProbe"),
    function(MEDIA_CODEC_PRIMARY_HASH, "emitPrimaryMediaCodecHash"),
    function(MEDIA_CODEC_PRIMARY_ERROR, "forwardPrimaryMediaCodecHashError"),
    function(MEDIA_CODEC_FALLBACK_HASH, "emitFallbackMediaCodecHash"),
    function(MEDIA_CODEC_FALLBACK_ERROR, "forwardFallbackMediaCodecHashError"),
    function(WEBRTC_CANDIDATES, "emitWebrtcIceCandidates"),
    function(WEBRTC_CANDIDATE_ERROR, "forwardWebrtcCandidateError"),
    function(CANVAS_FEATURE_COLLECTOR, "collectCanvasAndFeatureSignals"),
    function(CANVAS_PROBE_CLOSURE, "emitCanvasFingerprintSignals"),
    function(IMAGE_PROBE_STARTER, "startCanvasPixelProbe"),
    function(IMAGE_LOAD_HANDLER, "handleCanvasProbeImageLoad"),
    function(PIXEL_VECTOR_RECORDER, "recordPixelVectorSample"),
    function(PIXEL_VECTOR_UNIQUE, "acceptPixelVectorSample"),
    function(CANVAS_BLACK_SAMPLE, "collectBlackCanvasPixelSample"),
    function(CANVAS_DATA_URL_SAMPLE, "hashCanvasDataUrlSample"),
    function(CANVAS_PATH_SAMPLE, "sampleCanvasPathFill"),
    function(CANVAS_SEEDED_SAMPLE, "sampleSeededCanvasFill"),
    function(CANVAS_TEXT_SAMPLE, "sampleCanvasTextShadow"),
    function(CANVAS_QUEUE_ERROR, "forwardCanvasQueueError"),
    function(CANVAS_QUEUE_DIGEST, "emitCanvasPixelRecordDigest"),
    function(BROWSER_FINGERPRINT_COLLECTOR, "createBrowserFingerprintCollector"),
    function(BROWSER_FINGERPRINT_CLOSURE, "collectBrowserFingerprintSignals"),
    function(EMOJI_CHANNEL_SAMPLER, "sampleEmojiGlyphChannel"),
    function(EMOJI_RENDER_PROBE, "emitEmojiRenderProbe"),
    function(GLYPH_RENDER_PROBE, "emitGlyphRenderProbe"),
    function(BROWSER_PROBE_CLEANUP, "stopBrowserFingerprintProbes"),
    main(FORK_JOIN_ALIAS, "forkJoinObservableFactory"),
    main(REDUCE_OPERATOR_ALIAS, "reduceOperator"),
    main(ASPECT_RATIO_ALIAS, "aspectRatioProber"),
    main(CANVAS_GLYPH_SAMPLES, "canvasGlyphSamples"),
    main(EMOJI_RENDER_PROBES, "emojiRenderProbes"),
    main(NATIVE_CODE_CHECK_TARGETS, "nativeCodeCheckTargets"),
    main(HASH_OBSERVABLE_BINDER, "hashObservableBinder"),
    main(FONT_PROBE_CODE_POINTS, "fontProbeCodePoints"),
    main(FONT_PROBE_FAMILIES, "fontProbeFamilies"),
    main(STRING_FROM_CHAR_CODE, "stringFromCharCode"),
    main(MEDIA_CODEC_PROBE_STRINGS, "mediaCodecProbeStrings"),
    main(SLICE_ARRAY_LIKE, "sliceArrayLike"),
];

// ---------------------------------------------------------------------------
// Index
// ---------------------------------------------------------------------------

fn strip<'e, 'a>(expression: &'e Expression<'a>) -> &'e Expression<'a> {
    expression.without_parentheses()
}

/// `(object, name)` of a static or string-keyed member expression.
fn member_of<'e, 'a>(expression: &'e Expression<'a>) -> Option<(&'e Expression<'a>, &'e str)> {
    match strip(expression) {
        Expression::StaticMemberExpression(member) => {
            Some((&member.object, member.property.name.as_str()))
        }
        Expression::ComputedMemberExpression(member) => match &member.expression {
            Expression::StringLiteral(literal) => Some((&member.object, literal.value.as_str())),
            _ => None,
        },
        _ => None,
    }
}

fn target_member<'e, 'a>(
    target: &'e AssignmentTarget<'a>,
) -> Option<(&'e Expression<'a>, &'e str)> {
    match target {
        AssignmentTarget::StaticMemberExpression(member) => {
            Some((&member.object, member.property.name.as_str()))
        }
        AssignmentTarget::ComputedMemberExpression(member) => match &member.expression {
            Expression::StringLiteral(literal) => Some((&member.object, literal.value.as_str())),
            _ => None,
        },
        _ => None,
    }
}

fn call_arguments<'e, 'a>(call: &'e CallExpression<'a>) -> Vec<&'e Expression<'a>> {
    call.arguments.iter().filter_map(Argument::as_expression).collect()
}

fn string_value<'e>(expression: &'e Expression<'_>) -> Option<&'e str> {
    match strip(expression) {
        Expression::StringLiteral(literal) => Some(literal.value.as_str()),
        _ => None,
    }
}

fn is_boolean(expression: &Expression<'_>, value: bool) -> bool {
    matches!(strip(expression), Expression::BooleanLiteral(literal) if literal.value == value)
}

/// A call to a member method: `receiver.name(...)`.
struct MethodCall<'e, 'a> {
    receiver: &'e Expression<'a>,
    name: &'e str,
    arguments: Vec<&'e Expression<'a>>,
}

fn method_call<'e, 'a>(call: &'e CallExpression<'a>) -> Option<MethodCall<'e, 'a>> {
    let arguments = call_arguments(call);
    let (object, name) = member_of(&call.callee)?;
    Some(MethodCall { receiver: object, name, arguments })
}

#[derive(Clone, Copy)]
enum FnNode<'a> {
    Function(&'a Function<'a>),
    Arrow(&'a ArrowFunctionExpression<'a>),
}

struct Unit {
    node: NodeId,
    span: Span,
    parent: Option<usize>,
    is_declaration: bool,
}

/// What one function owns directly, excluding its nested functions.
#[derive(Default)]
struct Own<'a> {
    calls: Vec<&'a CallExpression<'a>>,
    news: Vec<&'a NewExpression<'a>>,
    returns: Vec<&'a Expression<'a>>,
    bare_returns: usize,
    /// Property names read or written anywhere in the function's own body.
    members: BTreeSet<String>,
    /// `receiver.name = value` writes.
    member_writes: Vec<(&'a Expression<'a>, &'a str, &'a Expression<'a>)>,
    /// Object literal keys written by the function's own body.
    written_keys: BTreeSet<String>,
    objects: Vec<&'a ObjectExpression<'a>>,
    /// Computed member reads: `(object, index)`.
    computed_members: Vec<(&'a Expression<'a>, &'a Expression<'a>)>,
    /// Symbol reads in the function's own body.
    reads: BTreeSet<SymbolId>,
    nested: Vec<usize>,
}

struct Index<'a> {
    scoping: &'a Scoping,
    nodes: &'a AstNodes<'a>,
    slots: ParameterSlots<'a, 'a>,
    fns: Vec<Unit>,
    own: Vec<Own<'a>>,
    owner: Vec<Option<usize>>,
    fn_of_node: BTreeMap<NodeId, usize>,
    decl_fn: BTreeMap<SymbolId, usize>,
    /// Initializer and plain `=` values of a binding.
    values: BTreeMap<SymbolId, Vec<&'a Expression<'a>>>,
    /// How many plain `=` assignments a binding has.
    assigned: BTreeMap<SymbolId, usize>,
    /// The plain `=` assignments of a binding, in node order.
    assignments: BTreeMap<SymbolId, Vec<&'a AssignmentExpression<'a>>>,
    /// Every call in the program, for sibling-argument lookup.
    all_calls: Vec<&'a CallExpression<'a>>,
}

impl<'a> Index<'a> {
    fn new(ctx: &FinderCtx<'_, 'a>) -> Self {
        let mut index = Self {
            scoping: ctx.scoping,
            nodes: ctx.nodes,
            slots: ParameterSlots::new(ctx.program, ctx.scoping, ctx.nodes),
            fns: Vec::new(),
            own: Vec::new(),
            owner: vec![None; ctx.nodes.len()],
            fn_of_node: BTreeMap::new(),
            decl_fn: BTreeMap::new(),
            values: BTreeMap::new(),
            assigned: BTreeMap::new(),
            assignments: BTreeMap::new(),
            all_calls: Vec::new(),
        };
        index.build();
        index
    }

    fn build(&mut self) {
        let nodes = self.nodes;
        for (id, node) in nodes.iter_enumerated() {
            let position = id.index();
            let parent = nodes.parent_id(id);
            let owner = if parent == id {
                None
            } else {
                self.fn_of_node.get(&parent).copied().or(self.owner[parent.index()])
            };
            self.owner[position] = owner;
            let func = match node.kind() {
                AstKind::Function(function) => Some((function.span, FnNode::Function(function))),
                AstKind::ArrowFunctionExpression(arrow) => {
                    Some((arrow.span, FnNode::Arrow(arrow)))
                }
                _ => None,
            };
            if let Some((span, func)) = func {
                let index = self.fns.len();
                let is_declaration = matches!(func, FnNode::Function(function) if function.is_declaration());
                let symbol = match func {
                    FnNode::Function(function) => {
                        function.id.as_ref().and_then(|id| id.symbol_id.get())
                    }
                    FnNode::Arrow(_) => None,
                };
                self.fns.push(Unit { node: id, span, parent: owner, is_declaration });
                self.own.push(Own::default());
                self.fn_of_node.insert(id, index);
                if let Some(owner) = owner {
                    self.own[owner].nested.push(index);
                }
                if let Some(symbol) = symbol {
                    self.decl_fn.insert(symbol, index);
                }
                if let FnNode::Arrow(arrow) = func {
                    if arrow.expression {
                        if let Some(Statement::ExpressionStatement(statement)) =
                            arrow.body.statements.first()
                        {
                            self.own[index].returns.push(&statement.expression);
                        }
                    }
                }
                continue;
            }
            self.index_node(node.kind(), owner);
        }
    }

    fn index_node(&mut self, kind: AstKind<'a>, owner: Option<usize>) {
        if let AstKind::CallExpression(call) = kind {
            self.all_calls.push(call);
        }
        match kind {
            AstKind::VariableDeclarator(declarator) => {
                if let (BindingPattern::BindingIdentifier(binding), Some(init)) =
                    (&declarator.id, declarator.init.as_ref())
                {
                    if let Some(symbol) = binding.symbol_id.get() {
                        self.values.entry(symbol).or_default().push(init);
                    }
                }
            }
            AstKind::AssignmentExpression(assignment)
                if assignment.operator == AssignmentOperator::Assign =>
            {
                if let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left {
                    if let Some(symbol) = analysis::identifier_symbol(identifier, self.scoping) {
                        self.values.entry(symbol).or_default().push(&assignment.right);
                        *self.assigned.entry(symbol).or_default() += 1;
                        self.assignments.entry(symbol).or_default().push(assignment);
                    }
                }
                if let Some(owner) = owner {
                    if let Some((object, name)) = target_member(&assignment.left) {
                        self.own[owner].member_writes.push((object, name, &assignment.right));
                        self.own[owner].members.insert(name.to_string());
                        self.own[owner].written_keys.insert(name.to_string());
                    }
                }
            }
            AstKind::ObjectProperty(property) if property.kind == PropertyKind::Init => {
                if let Some(name) = property_key_name(&property.key) {
                    if let Some(owner) = owner {
                        self.own[owner].written_keys.insert(name);
                    }
                }
            }
            _ => {}
        }
        let Some(owner) = owner else {
            return;
        };
        let scoping = self.scoping;
        let own = &mut self.own[owner];
        match kind {
            AstKind::CallExpression(call) => own.calls.push(call),
            AstKind::NewExpression(new) => own.news.push(new),
            AstKind::ReturnStatement(statement) => match &statement.argument {
                Some(argument) => own.returns.push(argument),
                None => own.bare_returns += 1,
            },
            AstKind::ObjectExpression(object) => own.objects.push(object),
            AstKind::StaticMemberExpression(found) => {
                own.members.insert(found.property.name.to_string());
            }
            AstKind::ComputedMemberExpression(found) => {
                if let Expression::StringLiteral(literal) = &found.expression {
                    own.members.insert(literal.value.to_string());
                } else {
                    own.computed_members.push((&found.object, &found.expression));
                }
            }
            AstKind::IdentifierReference(identifier) => {
                if let (Some(symbol), Some(reference)) = (
                    analysis::identifier_symbol(identifier, scoping),
                    identifier.reference_id.get(),
                ) {
                    if scoping.get_reference(reference).is_read() {
                        own.reads.insert(symbol);
                    }
                }
            }
            _ => {}
        }
    }

    // -----------------------------------------------------------------------
    // Resolution
    // -----------------------------------------------------------------------

    fn symbol(&self, expression: &Expression<'a>) -> Option<SymbolId> {
        analysis::expression_symbol(strip(expression), self.scoping)
    }

    /// A host member path rooted at an unbound global: `String.fromCharCode`.
    fn path(&self, expression: &Expression<'a>) -> Option<String> {
        match strip(expression) {
            Expression::Identifier(identifier) => (self.symbol(expression).is_none())
                .then(|| identifier.name.to_string()),
            _ => {
                let (object, name) = member_of(expression)?;
                Some(format!("{}.{name}", self.path(object)?))
            }
        }
    }

    /// Every source origin of a binding, or `None` when a write is not a
    /// declarator initializer or a plain assignment.
    fn origins(&self, symbol: SymbolId) -> Option<Vec<&'a Expression<'a>>> {
        if self.scoping.symbol_declarations(symbol).count() != 1 {
            return None;
        }
        let assignments = self.assigns(&symbol);
        let writes = self
            .scoping
            .get_resolved_references(symbol)
            .filter(|reference| reference.is_write())
            .count();
        if writes != assignments.len() {
            return None;
        }
        let mut out = Vec::new();
        if let AstKind::VariableDeclarator(declarator) =
            self.nodes.kind(self.scoping.symbol_declaration(symbol))
        {
            if let Some(init) = declarator.init.as_ref() {
                out.push(init);
            }
        }
        out.extend(assignments.iter().map(|assignment| &assignment.right));
        Some(out)
    }

    fn assigns(&self, symbol: &SymbolId) -> Vec<&'a AssignmentExpression<'a>> {
        self.assignments.get(symbol).cloned().unwrap_or_default()
    }

    fn single_origin(&self, symbol: SymbolId) -> Option<&'a Expression<'a>> {
        match self.origins(symbol)?.as_slice() {
            [origin] => Some(*origin),
            _ => None,
        }
    }

    fn function_of(&self, expression: &Expression<'a>) -> Option<usize> {
        self.function_of_depth(expression, 8)
    }

    fn function_of_depth(&self, expression: &Expression<'a>, depth: u8) -> Option<usize> {
        match strip(expression) {
            Expression::FunctionExpression(found) => {
                return self.fn_of_node.get(&found.node_id.get()).copied();
            }
            Expression::ArrowFunctionExpression(found) => {
                return self.fn_of_node.get(&found.node_id.get()).copied();
            }
            _ => {}
        }
        if depth == 0 {
            return None;
        }
        let symbol = self.symbol(expression)?;
        if let Some(index) = self.decl_fn.get(&symbol).copied() {
            let immutable = self.scoping.symbol_declarations(symbol).count() == 1
                && !self.scoping.get_resolved_references(symbol).any(|r| r.is_write());
            return immutable.then_some(index);
        }
        self.function_of_depth(self.single_origin(symbol)?, depth - 1)
    }

    /// The parameter slot of `f` an expression reads, through single-value
    /// locals of `f`.
    fn slot(&self, expression: &Expression<'a>, f: usize) -> Option<u32> {
        let mut current = strip(expression);
        for _ in 0..8 {
            if let Some(slot) = self.slots.incoming_slot(current) {
                return (slot.owner == self.fns[f].node).then_some(slot.index);
            }
            let symbol = self.symbol(current)?;
            current = strip(self.single_origin(symbol)?);
        }
        None
    }

    fn is_slot(&self, expression: &Expression<'a>, f: usize, index: u32) -> bool {
        self.slot(expression, f) == Some(index)
    }

    /// Every function and arrow nested in `f`, including `f`.
    fn subtree(&self, f: usize) -> Vec<usize> {
        (0..self.fns.len())
            .filter(|candidate| {
                let mut current = Some(*candidate);
                while let Some(index) = current {
                    if index == f {
                        return true;
                    }
                    current = self.fns[index].parent;
                }
                false
            })
            .collect()
    }

    fn reads_under(&self, f: usize, out: &mut BTreeSet<SymbolId>) {
        out.extend(self.own[f].reads.iter().copied());
        for nested in &self.own[f].nested {
            self.reads_under(*nested, out);
        }
    }

    /// Whether a binding is declared outside `f` (a captured MAIN value).
    fn declared_outside(&self, symbol: SymbolId, f: usize) -> bool {
        let declaration = self.nodes.get_node(self.scoping.symbol_declaration(symbol)).span();
        let span = self.fns[f].span;
        declaration.start < span.start || declaration.end > span.end
    }

    /// Whether a node is lexically inside a `catch` block of its own function.
    fn inside_catch(&self, node: NodeId) -> bool {
        let mut current = node;
        for _ in 0..512 {
            let parent = self.nodes.parent_id(current);
            if parent == current {
                return false;
            }
            current = parent;
            match self.nodes.kind(current) {
                AstKind::CatchClause(_) => return true,
                AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) => return false,
                _ => {}
            }
        }
        false
    }

    fn callback_arguments(&self, call: &CallExpression<'a>) -> Vec<usize> {
        call_arguments(call)
            .into_iter()
            .filter_map(|argument| self.function_of(argument))
            .collect()
    }

    /// The one function `f` returns, when it has exactly one return.
    fn returned_function(&self, f: usize) -> Option<usize> {
        if self.own[f].bare_returns != 0 || self.own[f].returns.len() != 1 {
            return None;
        }
        self.function_of(self.own[f].returns[0])
    }

    /// The `[object, object.method]; temp[1].call(temp[0], callback)` idiom the
    /// emitted captures use instead of a direct `object.method(callback)`.
    /// Returns every callback handed to such a forwarding call.
    fn forwarded_callbacks(&self, f: usize) -> Vec<usize> {
        let mut out = Vec::new();
        for call in &self.own[f].calls {
            let arguments = call_arguments(call);
            let Some((method_object, "call")) = member_of(&call.callee) else {
                continue;
            };
            let Expression::ComputedMemberExpression(method) = strip(method_object) else {
                continue;
            };
            let Some(first) = arguments.first() else {
                continue;
            };
            let Expression::ComputedMemberExpression(receiver) = strip(first) else {
                continue;
            };
            if numeric_index(&method.expression) != Some(1)
                || numeric_index(&receiver.expression) != Some(0)
            {
                continue;
            }
            let (Some(method_object), Some(receiver_object)) =
                (self.symbol(&method.object), self.symbol(&receiver.object))
            else {
                continue;
            };
            if method_object != receiver_object {
                continue;
            }
            for argument in arguments.iter().skip(1) {
                if let Some(callback) = self.function_of(argument) {
                    out.push(callback);
                }
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Claims
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Out {
    claims: Vec<FinderClaim>,
    seen: BTreeSet<(&'static str, u32, u32)>,
}

impl Out {
    fn function(&mut self, ix: &Index<'_>, role: &'static str, index: usize, evidence: &str) {
        let span = ix.fns[index].span;
        if self.seen.insert((role, span.start, span.end)) {
            self.claims.push(FinderClaim::Function {
                role,
                function_span: span,
                alias_role: None,
                confidence: Confidence::Medium,
                evidence: vec![evidence.to_string()],
            });
        }
    }

    fn function_with_alias(
        &mut self,
        ix: &Index<'_>,
        role: &'static str,
        index: usize,
        alias_role: &'static str,
        evidence: &str,
    ) {
        let span = ix.fns[index].span;
        if self.seen.insert((role, span.start, span.end)) {
            self.claims.push(FinderClaim::Function {
                role,
                function_span: span,
                alias_role: Some(alias_role),
                confidence: Confidence::Medium,
                evidence: vec![evidence.to_string()],
            });
        }
    }

    fn binding(&mut self, ix: &Index<'_>, role: &'static str, symbol: SymbolId, evidence: &str) {
        let span = ix.scoping.symbol_span(symbol);
        if self.seen.insert((role, span.start, span.end)) {
            self.claims.push(FinderClaim::MainBinding {
                role,
                identifier_span: span,
                confidence: Confidence::Medium,
                evidence: vec![evidence.to_string()],
            });
        }
    }

    /// The claims of every role that matched exactly one span. Two candidates
    /// for one role cannot be told apart, so neither is named.
    /// The claims of every role that matched exactly one span. Two candidates
    /// for one role cannot be told apart, so neither is named. A span claimed
    /// by two different roles is dropped as well: the plan would refuse the
    /// role conflict and the binding would keep its original spelling, which is
    /// a regression when a capture merges two probe shapes into one function.
    fn into_unique(self) -> Vec<FinderClaim> {
        let mut spans: BTreeMap<&'static str, usize> = BTreeMap::new();
        let mut roles_by_span: BTreeMap<(u32, u32), BTreeSet<&'static str>> = BTreeMap::new();
        for (role, start, end) in &self.seen {
            *spans.entry(*role).or_default() += 1;
            roles_by_span.entry((*start, *end)).or_default().insert(*role);
        }
        let mut claims = self.claims;
        claims.retain(|claim| {
            let span = match claim {
                FinderClaim::Function { function_span, .. } => {
                    (function_span.start, function_span.end)
                }
                FinderClaim::MainBinding { identifier_span, .. } => {
                    (identifier_span.start, identifier_span.end)
                }
                FinderClaim::ClassBinding { class_span, .. } => {
                    (class_span.start, class_span.end)
                }
            };
            spans.get(claim.role()) == Some(&1)
                && roles_by_span.get(&span).is_some_and(|roles| roles.len() == 1)
        });
        claims
    }
}

/// Structural claims of this package.
pub(crate) fn find(ctx: &FinderCtx) -> Vec<FinderClaim> {
    let ix = Index::new(ctx);
    let mut out = Out::default();
    let mut proven = Proven::default();
    emoji_canvas(&ix, &mut out);
    webgl_probe(&ix, &mut out, &mut proven);
    font_metrics_probe(&ix, &mut out);
    seeded_font_metrics(&ix, &mut out, &mut proven);
    media_codec_probe(&ix, &mut out, &mut proven);
    webrtc_candidates(&ix, &mut out);
    canvas_feature_collector(&ix, &mut out, &mut proven);
    browser_fingerprint_collector(&ix, &mut out, &mut proven);
    main_bindings(&ix, &mut out, &proven);
    out.into_unique()
}

/// The functions whose consumers prove a MAIN binding.
#[derive(Default)]
struct Proven {
    font_family_metrics: Option<usize>,
    seeded_font_probe: Option<usize>,
    media_codec_probe: Option<usize>,
    webgl_probe: Option<usize>,
    canvas_closure: Option<usize>,
    browser_closure: Option<usize>,
}

// ---------------------------------------------------------------------------
// Emoji canvas collector (group 0)
// ---------------------------------------------------------------------------

fn emoji_canvas(ix: &Index<'_>, out: &mut Out) {
    for f in 0..ix.fns.len() {
        let own = &ix.own[f];
        // A canvas is created in the body and an image/sampling callback is
        // handed to a queue factory.
        let creates_canvas = own.calls.iter().any(|call| {
            method_call(call).is_some_and(|method| {
                method.name == "createElement"
                    && method.arguments.first().and_then(|a| string_value(a)) == Some("canvas")
            })
        });
        if !creates_canvas {
            continue;
        }
        let sampler = own.calls.iter().find_map(|call| {
            ix.callback_arguments(call)
                .into_iter()
                .find(|callback| {
                    let nested = &ix.own[*callback];
                    nested.members.contains("getImageData")
                        && nested.members.contains("getContext")
                        && nested.members.contains("fillText")
                })
        });
        let teardown = ix.returned_function(f);
        let Some(sampler) = sampler else {
            continue;
        };
        let Some(teardown) = teardown else {
            continue;
        };
        if !ix.own[teardown].calls.iter().any(|call| {
            method_call(call).is_some_and(|method| method.name == "empty")
        }) {
            continue;
        }
        // The queue's `onEmpty` callback hands the accumulated samples to the
        // collector's sink slot.
        let emitter = ix
            .forwarded_callbacks(f)
            .into_iter()
            .chain(ix.own[f].calls.iter().flat_map(|call| ix.callback_arguments(call)))
            .find(|callback| {
                ix.own[*callback].calls.iter().any(|call| {
                    ix.slot(&call.callee, f) == Some(3)
                })
            });
        out.function_with_alias(
            ix,
            EMOJI_CANVAS_COLLECTOR,
            f,
            EMOJI_CANVAS_ALIAS,
            "creates a canvas, schedules a glyph-sampling task queue over it and returns a teardown that empties the queue",
        );
        out.function(
            ix,
            EMOJI_CANVAS_SAMPLER,
            sampler,
            "queue runner that draws one glyph with fillText and samples its pixels through getImageData",
        );
        if let Some(emitter) = emitter {
            out.function(
                ix,
                EMOJI_CANVAS_EMITTER,
                emitter,
                "onEmpty callback of the glyph queue; hands the accumulated sample arrays to the collector's sink argument",
            );
        }
        out.function(
            ix,
            EMOJI_CANVAS_TEARDOWN,
            teardown,
            "returned teardown that calls empty() on the glyph queue",
        );
        return;
    }
}

// ---------------------------------------------------------------------------
// WebGL pixel probe
// ---------------------------------------------------------------------------

fn webgl_probe(ix: &Index<'_>, out: &mut Out, proven: &mut Proven) {
    for f in 0..ix.fns.len() {
        let own = &ix.own[f];
        let gets_webgl = own.calls.iter().any(|call| {
            method_call(call).is_some_and(|method| {
                method.name == "getContext"
                    && method
                        .arguments
                        .first()
                        .and_then(|argument| string_value(argument))
                        .is_some_and(|name| name.starts_with("webgl") || name.starts_with("experimental-webgl"))
            })
        });
        if !gets_webgl || !own.members.contains("readPixels") || !own.members.contains("createShader")
        {
            continue;
        }
        // A capture may merge the emoji glyph sampler and the WebGL probe into
        // one function. p05 already owns that shape as the emoji glyph pixel
        // collector, so this package leaves it alone instead of forcing a role
        // conflict: the pure WebGL probe does not draw glyph text.
        if own.members.contains("fillText") && own.members.contains("getImageData") {
            continue;
        }
        out.function(
            ix,
            WEBGL_PROBE,
            f,
            "creates a canvas, gets a webgl context, compiles shaders and reads back its pixels",
        );
        proven.webgl_probe = Some(f);
        // The pixel bytes are handed to the hash dispatcher, whose onValue and
        // onError callbacks are the sibling arguments.
        for call in &own.calls {
            let arguments = call_arguments(call);
            let [bytes, _, value, error] = arguments.as_slice() else {
                continue;
            };
            let Some(symbol) = ix.symbol(bytes) else {
                continue;
            };
            let allocates_bytes = ix.single_origin(symbol).is_some_and(|origin| {
                matches!(strip(origin), Expression::NewExpression(new)
                    if member_of(&new.callee).is_some_and(|(_, name)| name == "Uint8Array")
                        || matches!(strip(&new.callee), Expression::Identifier(identifier) if identifier.name == "Uint8Array"))
            });
            if !allocates_bytes {
                continue;
            }
            if let Some(value) = ix.function_of(value) {
                out.function(
                    ix,
                    WEBGL_PIXEL_HASH_EMITTER,
                    value,
                    "onValue callback of the WebGL pixel hash; writes masked marker bytes and emits the digest",
                );
            }
            if let Some(error) = ix.function_of(error) {
                out.function(
                    ix,
                    WEBGL_PIXEL_HASH_ERROR,
                    error,
                    "onError callback of the WebGL pixel hash; forwards the error to the group's observer",
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Font metrics probe (group with measureText queue)
// ---------------------------------------------------------------------------

/// A queue local: a symbol whose single source origin is a call.
fn queue_local(ix: &Index<'_>, span: Span) -> Option<SymbolId> {
    let mut found = None;
    for (symbol, values) in &ix.values {
        if values.len() == 1 && values[0].span() == span {
            found = Some(*symbol);
        }
    }
    found
}

fn font_metrics_probe(ix: &Index<'_>, out: &mut Out) {
    for f in 0..ix.fns.len() {
        let own = &ix.own[f];
        if !own.members.contains("measureText") || !own.members.contains("onError") {
            continue;
        }
        let Some((queue_span, runner)) = own.calls.iter().find_map(|call| {
            let callback = ix.callback_arguments(call).into_iter().find(|callback| {
                ix.own[*callback].members.contains("measureText")
            })?;
            Some((call.span, callback))
        }) else {
            continue;
        };
        out.function(
            ix,
            FONT_METRICS_PROBE,
            f,
            "creates a canvas, hashes measureText widths and schedules a font-width task queue",
        );
        out.function(
            ix,
            FONT_METRIC_WIDTH_RUNNER,
            runner,
            "font queue runner that compares measureText(width) of sans-serif and serif fonts and emits the hash",
        );
        let Some(queue) = queue_local(ix, queue_span) else {
            continue;
        };
        for call in &own.calls {
            let Some(method) = method_call(call) else {
                continue;
            };
            if ix.symbol(method.receiver) != Some(queue) {
                continue;
            }
            let Some(callback) = method.arguments.first().and_then(|arg| ix.function_of(arg)) else {
                continue;
            };
            match method.name {
                "onError" => out.function(
                    ix,
                    FONT_METRICS_QUEUE_ERROR,
                    callback,
                    "onError callback of the font-width queue; throws the task queue error",
                ),
                "onEmpty" => out.function(
                    ix,
                    FONT_METRICS_QUEUE_EMPTY,
                    callback,
                    "onEmpty callback of the font-width queue; completes the group observer",
                ),
                _ => {}
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Seeded font-metric digest (families.map(codePoints.reduce(...)))
// ---------------------------------------------------------------------------

fn seeded_font_metrics(ix: &Index<'_>, out: &mut Out, proven: &mut Proven) {
    for f in 0..ix.fns.len() {
        let own = &ix.own[f];
        // The probe creates a canvas and subscribes an observer to a fork-joined
        // observable built from the font family list.
        let subscribes = own.calls.iter().any(|call| {
            method_call(call).is_some_and(|method| {
                method.name == "subscribe" && !method.arguments.is_empty()
            })
        });
        let creates_canvas = own.calls.iter().any(|call| {
            method_call(call).is_some_and(|method| {
                method.name == "createElement"
                    && method.arguments.first().and_then(|a| string_value(a)) == Some("canvas")
            }) || own.news.iter().any(|new| {
                matches!(strip(&new.callee), Expression::Identifier(identifier)
                    if identifier.name == "OffscreenCanvas")
            })
        });
        if !(subscribes && creates_canvas) {
            continue;
        }
        // The observer object literal carries a `next` member.
        let observer = own.calls.iter().find_map(|call| {
            let method = method_call(call)?;
            if method.name != "subscribe" {
                return None;
            }
            let argument = method.arguments.first()?;
            let symbol = ix.symbol(argument)?;
            let origin = ix.single_origin(symbol)?;
            let Expression::ObjectExpression(object) = strip(origin) else {
                return None;
            };
            object_members_of(object)
                .into_iter()
                .find(|member| member.name == "next")
                .and_then(|member| ix.function_of(member.value))
                .map(|next| (symbol, next))
        });
        let Some((observer_symbol, next)) = observer else {
            continue;
        };
        out.function(
            ix,
            SEEDED_FONT_METRICS_PROBE,
            f,
            "builds a canvas and subscribes the font-family fork-join observable",
        );
        out.function(
            ix,
            SEEDED_FONT_METRICS_DIGEST,
            next,
            "next handler of the seeded font-metric observer; hashes the collected strings and completes",
        );
        // The observer's error member is assigned next to the literal.
        for (object, name, value) in &own.member_writes {
            if name == &"error" && ix.symbol(object) == Some(observer_symbol) {
                if let Some(error) = ix.function_of(value) {
                    out.function(
                        ix,
                        FONT_PROBE_ERROR,
                        error,
                        "error handler of the seeded font-metric observer; forwards the error",
                    );
                }
            }
        }
        // The family list is mapped to one observable per family.
        for call in &own.calls {
            let Some(method) = method_call(call) else {
                continue;
            };
            if method.name != "map" {
                continue;
            }
            let Some(family) = method.arguments.first().and_then(|arg| ix.function_of(arg)) else {
                continue;
            };
            if !ix.own[family].calls.iter().any(|call| {
                method_call(call).is_some_and(|method| method.name == "pipe")
            }) {
                continue;
            }
            out.function(
                ix,
                FONT_FAMILY_METRICS,
                family,
                "map callback over the font families; reduces the code-point list to hex text and hashes it",
            );
            proven.font_family_metrics = Some(family);
            // The reducer and the hash mapper are nested in the family callback.
            for nested in ix.subtree(family) {
                let nested_own = &ix.own[nested];
                if nested_own.members.contains("measureText") {
                    out.function(
                        ix,
                        FONT_METRIC_REDUCER,
                        nested,
                        "reduce callback that measures one code point and appends its hex width and bounding box",
                    );
                } else if nested_own.calls.len() == 1
                    && nested_own.returns.len() == 1
                    && matches!(strip(nested_own.returns[0]), Expression::CallExpression(call)
                        if call_arguments(call).first().is_some_and(|arg| ix.is_slot(arg, nested, 0)))
                {
                    out.function(
                        ix,
                        FONT_METRIC_HASH_MAPPER,
                        nested,
                        "map callback hashing the accumulated font-metric text of its family",
                    );
                }
            }
        }
        proven.seeded_font_probe = Some(f);
        return;
    }
}

// ---------------------------------------------------------------------------
// Media-codec probe
// ---------------------------------------------------------------------------

fn media_codec_probe(ix: &Index<'_>, out: &mut Out, proven: &mut Proven) {
    for f in 0..ix.fns.len() {
        let own = &ix.own[f];
        let asks_codecs = own.calls.iter().any(|call| {
            method_call(call).is_some_and(|method| method.name == "canPlayType")
        });
        if !asks_codecs
            || (!own.members.contains("featurePolicy") && !own.members.contains("getAllowlistForFeature"))
        {
            continue;
        }
        out.function(
            ix,
            MEDIA_CODEC_PROBE,
            f,
            "probes a video element's canPlayType over the codec list and hashes a feature-policy report",
        );
        proven.media_codec_probe = Some(f);
        // Both the primary and the catch path hash the canvas data URL.
        for call in &own.calls {
            let arguments = call_arguments(call);
            let [data, _, value, error] = arguments.as_slice() else {
                continue;
            };
            let Some(symbol) = ix.symbol(data) else {
                continue;
            };
            let is_data_url = ix.origins(symbol).is_some_and(|origins| {
                origins.iter().any(|origin| {
                    matches!(strip(origin), Expression::CallExpression(call)
                        if method_call(call).is_some_and(|method| method.name == "toDataURL"))
                })
            });
            if !is_data_url {
                continue;
            }
            let fallback = ix.inside_catch(call.node_id.get());
            let value_role = if fallback { MEDIA_CODEC_FALLBACK_HASH } else { MEDIA_CODEC_PRIMARY_HASH };
            let error_role = if fallback { MEDIA_CODEC_FALLBACK_ERROR } else { MEDIA_CODEC_PRIMARY_ERROR };
            if let Some(value) = ix.function_of(value) {
                out.function(
                    ix,
                    value_role,
                    value,
                    if fallback {
                        "onValue callback of the catch-path canvas data URL hash"
                    } else {
                        "onValue callback of the primary canvas data URL hash"
                    },
                );
            }
            if let Some(error) = ix.function_of(error) {
                out.function(
                    ix,
                    error_role,
                    error,
                    if fallback {
                        "onError callback of the catch-path canvas data URL hash"
                    } else {
                        "onError callback of the primary canvas data URL hash"
                    },
                );
            }
        }
        return;
    }
}

// ---------------------------------------------------------------------------
// WebRTC candidates
// ---------------------------------------------------------------------------

fn webrtc_candidates(ix: &Index<'_>, out: &mut Out) {
    let candidates: Vec<usize> = (0..ix.fns.len())
        .filter(|f| {
            let own = &ix.own[*f];
            own.members.contains("ip")
                && own.members.contains("candidate")
                && own.calls.iter().any(|call| {
                    method_call(call).is_some_and(|method| method.name == "next")
                })
        })
        .collect();
    for candidate in candidates {
        out.function(
            ix,
            WEBRTC_CANDIDATES,
            candidate,
            "success callback that hashes each gathered ICE candidate's ip and candidate strings",
        );
        // The sibling argument of the same call is the error handler.
        for call in &ix.all_calls {
            let arguments = call_arguments(call);
            let Some(position) = arguments
                .iter()
                .position(|argument| ix.function_of(argument) == Some(candidate))
            else {
                continue;
            };
            if let Some(error) = arguments.get(position + 1).and_then(|arg| ix.function_of(arg)) {
                out.function(
                    ix,
                    WEBRTC_CANDIDATE_ERROR,
                    error,
                    "error callback of the same ICE-candidate probe; forwards the error",
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Canvas and feature-signal collector (group 4)
// ---------------------------------------------------------------------------

/// `registrar(closure, flag)` in `f`'s own body.
fn registration(ix: &Index<'_>, f: usize, flag: bool) -> Option<(Span, usize)> {
    for call in &ix.own[f].calls {
        let arguments = call_arguments(call);
        let [first, second] = arguments.as_slice() else {
            continue;
        };
        if !is_boolean(second, flag) {
            continue;
        }
        if let Some(closure) = ix.function_of(first) {
            return Some((call.span, closure));
        }
    }
    None
}

/// `registrar(closure)` with a single function argument.
fn single_registration(ix: &Index<'_>, f: usize) -> Option<usize> {
    for call in &ix.own[f].calls {
        let arguments = call_arguments(call);
        let [only] = arguments.as_slice() else {
            continue;
        };
        if let Some(closure) = ix.function_of(only) {
            return Some(closure);
        }
    }
    None
}

fn canvas_feature_collector(ix: &Index<'_>, out: &mut Out, proven: &mut Proven) {
    // The collection closure is the one that enqueues several tasks on a
    // sequential queue captured from its parent collector body. No other group
    // closure enqueues work itself.
    for closure in 0..ix.fns.len() {
        let mut queue: Option<SymbolId> = None;
        let mut enqueued = 0usize;
        for call in &ix.own[closure].calls {
            let Some(method) = method_call(call) else {
                continue;
            };
            if method.name != "enqueue" {
                continue;
            }
            let Some(receiver) = ix.symbol(method.receiver) else {
                continue;
            };
            if !ix.declared_outside(receiver, closure) {
                continue;
            }
            queue = Some(receiver);
            enqueued += 1;
        }
        let Some(queue) = queue else {
            continue;
        };
        if enqueued < 2 {
            continue;
        }
        let Some(parent) = ix.fns[closure].parent else {
            continue;
        };
        let Some((_, registered)) = registration(ix, parent, false) else {
            continue;
        };
        if registered != closure {
            continue;
        }
        // A grouping body with a separate cleanup is not enough: an environment
        // collector has the same registration shape. Require this package's own
        // canvas proof in the registered closure, the way the emoji collector
        // requires its glyph queue, so a merged host-fingerprint collector is
        // left to the package that owns its host calls.
        if !closure_draws_canvas(ix, closure) {
            continue;
        }
        out.function(
            ix,
            CANVAS_FEATURE_COLLECTOR,
            parent,
            "refills and shifts the MAIN seed pool, then registers the canvas collection closure and its init closure",
        );
        out.function(
            ix,
            CANVAS_PROBE_CLOSURE,
            closure,
            "collection closure registered with the grouping flag off; draws the canvas scene and enqueues the pixel-sample tasks",
        );
        proven.canvas_closure = Some(closure);
        claim_canvas_closure(ix, out, closure, queue);
        return;
    }
}

/// The registered closure proves this package's canvas fingerprint: it takes a
/// 2d context, or draws/samples a canvas directly or in a task it enqueues.
fn closure_draws_canvas(ix: &Index<'_>, closure: usize) -> bool {
    ix.subtree(closure).into_iter().any(|function| {
        let own = &ix.own[function];
        own.calls.iter().any(|call| {
            method_call(call).is_some_and(|method| {
                method.name == "getContext"
                    && method.arguments.first().and_then(|argument| string_value(argument))
                        == Some("2d")
            })
        }) || CANVAS_DRAW_MEMBERS.iter().any(|name| own.members.contains(*name))
    })
}

/// Canvas 2d drawing/sampling members that prove a canvas feature collector.
const CANVAS_DRAW_MEMBERS: &[&str] = &[
    "fillText",
    "fillRect",
    "strokeText",
    "drawImage",
    "getImageData",
    "putImageData",
    "toDataURL",
];

fn claim_canvas_closure(ix: &Index<'_>, out: &mut Out, closure: usize, queue: SymbolId) {
    // The image probe starter and recorder are declared inside the closure.
    for nested in ix.subtree(closure) {
        let own = &ix.own[nested];
        let onload = own.member_writes.iter().find_map(|(_, name, value)| {
            (*name == "onload").then(|| ix.function_of(value)).flatten()
        });
        let assigns_src = own.member_writes.iter().any(|(_, name, _)| *name == "src");
        if own.news.len() == 1 && assigns_src && onload.is_some() {
            out.function(
                ix,
                IMAGE_PROBE_STARTER,
                nested,
                "starts a canvas image probe: creates a 5x5 canvas and an Image whose src is the data URL",
            );
            if let Some(handler) = onload.filter(|handler| {
                ix.own[*handler].members.contains("drawImage")
            }) {
                out.function(
                    ix,
                    IMAGE_LOAD_HANDLER,
                    handler,
                    "image onload handler; draws the loaded image, samples 4-byte pixels and emits after four loads",
                );
            }
        }
        let records_samples = own.calls.iter().any(|call| {
            method_call(call).is_some_and(|method| method.name == "some")
        }) && own.members.contains("push")
            && own.members.contains("length");
        if records_samples {
            out.function(
                ix,
                PIXEL_VECTOR_RECORDER,
                nested,
                "records one pixel-vector sample: dedupes, bumps the sample counter and pushes the slice",
            );
            for call in &own.calls {
                let Some(method) = method_call(call) else {
                    continue;
                };
                if method.name != "some" {
                    continue;
                }
                if let Some(unique) = method.arguments.first().and_then(|arg| ix.function_of(arg)) {
                    out.function(
                        ix,
                        PIXEL_VECTOR_UNIQUE,
                        unique,
                        "some() predicate of the pixel-vector recorder; computes element equality but always accepts the sample",
                    );
                }
            }
        }
    }
    // The sequential queue is captured from the collector body; its five
    // enqueued tasks, onError and onEmpty are the closure's own call graph.
    for call in &ix.own[closure].calls {
        let Some(method) = method_call(call) else {
            continue;
        };
        if ix.symbol(method.receiver) != Some(queue) {
            continue;
        }
        let Some(callback) = method.arguments.first().and_then(|arg| ix.function_of(arg)) else {
            continue;
        };
        match method.name {
            "enqueue" => claim_canvas_task(ix, out, callback),
            "onError" => out.function(
                ix,
                CANVAS_QUEUE_ERROR,
                callback,
                "onError callback of the sequential canvas queue; forwards the error to the group observer",
            ),
            "onEmpty" => out.function(
                ix,
                CANVAS_QUEUE_DIGEST,
                callback,
                "onEmpty callback of the sequential canvas queue; hashes the four pixel records and completes",
            ),
            _ => {}
        }
    }
}

fn claim_canvas_task(ix: &Index<'_>, out: &mut Out, task: usize) {
    let own = &ix.own[task];
    let has_text = own.members.contains("fillText")
        || own.members.contains("shadowBlur")
        || own.members.contains("shadowOffsetX");
    let has_path = own.calls.iter().any(|call| {
        method_call(call).is_some_and(|method| {
            matches!(method.name, "moveTo" | "lineTo" | "closePath" | "fill")
        })
    });
    let has_data_url = own.calls.iter().any(|call| {
        method_call(call).is_some_and(|method| method.name == "toDataURL")
    });
    let has_fill = own.members.contains("fillRect") || own.members.contains("fillStyle");
    let (role, evidence) = if has_text {
        (
            CANVAS_TEXT_SAMPLE,
            "canvas queue task that draws text/shadow through fillText and samples the result",
        )
    } else if has_path {
        (
            CANVAS_PATH_SAMPLE,
            "canvas queue task that fills a path through moveTo/lineTo then samples it",
        )
    } else if has_data_url && !has_fill {
        (
            CANVAS_DATA_URL_SAMPLE,
            "canvas queue task that hashes the canvas data URL and starts the image reload probe",
        )
    } else if has_data_url && has_fill {
        (
            CANVAS_SEEDED_SAMPLE,
            "canvas queue task that fills with a seed-derived rgba color and samples the result",
        )
    } else if has_fill {
        (
            CANVAS_BLACK_SAMPLE,
            "canvas queue task that fills the canvas black and samples it",
        )
    } else {
        return;
    };
    out.function(ix, role, task, evidence);
}

// ---------------------------------------------------------------------------
// Browser fingerprint collector (group 0 body with the true registration flag)
// ---------------------------------------------------------------------------

fn browser_fingerprint_collector(ix: &Index<'_>, out: &mut Out, proven: &mut Proven) {
    for f in 0..ix.fns.len() {
        let Some((_, closure)) = registration(ix, f, true) else {
            continue;
        };
        let Some(cleanup) = single_registration(ix, f) else {
            continue;
        };
        if !ix.own[cleanup].calls.iter().any(|call| {
            method_call(call).is_some_and(|method| method.name == "empty")
        }) {
            continue;
        }
        if !ix.fns[f].is_declaration {
            continue;
        }
        // A group body with a separate cleanup is not enough: the touch and
        // device-orientation collectors have the same registration shape and
        // p10 owns them. Require the closure to run this package's emoji
        // pixel-sampling queue, which is the canvas-fingerprint proof.
        if !closure_samples_emoji(ix, closure) {
            continue;
        }
        out.function(
            ix,
            BROWSER_FINGERPRINT_COLLECTOR,
            f,
            "registers the browser-fingerprint collection closure with the grouping flag on and a separate cleanup closure",
        );
        out.function(
            ix,
            BROWSER_FINGERPRINT_CLOSURE,
            closure,
            "collection closure registered with the grouping flag on; runs the canvas, emoji, glyph, media and WebGL probes",
        );
        out.function(
            ix,
            BROWSER_PROBE_CLEANUP,
            cleanup,
            "cleanup closure that stops the worker and empties the emoji and glyph queues",
        );
        proven.browser_closure = Some(closure);
        claim_browser_closure(ix, out, closure);
        return;
    }
}

/// The closure feeds the emoji probe table to a canvas sampling runner.
fn closure_samples_emoji(ix: &Index<'_>, closure: usize) -> bool {
    ix.own[closure].calls.iter().any(|call| {
        let arguments = call_arguments(call);
        let [runner, _, table] = arguments.as_slice() else {
            return false;
        };
        let Some(runner) = ix.function_of(runner) else {
            return false;
        };
        let Some(table) = ix.symbol(table) else {
            return false;
        };
        ix.own[runner].members.contains("getImageData")
            && ix.own[runner].members.contains("fillText")
            && is_emoji_probe_table(ix, table)
    })
}

fn claim_browser_closure(ix: &Index<'_>, out: &mut Out, closure: usize) {
    // The emoji queue is fed the `[glyph, x, y, channel]` probe table; its
    // runner samples one channel of one pixel per task.
    for call in &ix.own[closure].calls {
        let arguments = call_arguments(call);
        let [runner, _, table] = arguments.as_slice() else {
            continue;
        };
        let Some(runner) = ix.function_of(runner) else {
            continue;
        };
        if !ix.own[runner].members.contains("getImageData")
            || !ix.own[runner].members.contains("fillText")
        {
            continue;
        }
        let Some(table) = ix.symbol(table) else {
            continue;
        };
        if !is_emoji_probe_table(ix, table) {
            continue;
        }
        out.function(
            ix,
            EMOJI_CHANNEL_SAMPLER,
            runner,
            "emoji queue runner that draws one glyph and samples one channel of one pixel, recording -1 on failure",
        );
        out.binding(
            ix,
            EMOJI_RENDER_PROBES,
            table,
            "array of [glyph, x, y, channel] rows handed to the emoji sampling queue",
        );
        // The emoji queue local is the one whose call takes this runner.
        let queue = queue_local(ix, call.span);
        let Some(queue) = queue else {
            continue;
        };
        for nested_call in &ix.own[closure].calls {
            let Some(method) = method_call(nested_call) else {
                continue;
            };
            if method.name != "onEmpty" || ix.symbol(method.receiver) != Some(queue) {
                continue;
            }
            if let Some(callback) = method.arguments.first().and_then(|arg| ix.function_of(arg)) {
                out.function(
                    ix,
                    EMOJI_RENDER_PROBE,
                    callback,
                    "onEmpty callback of the emoji queue; hashes both halves of the accumulated glyph samples",
                );
            }
        }
    }
    // The glyph queue receives the `[glyph, [[x, y], ...]]` table and an
    // onEmpty callback that hashes the rows.
    for call in &ix.own[closure].calls {
        let arguments = call_arguments(call);
        let [_, _, table, callback] = arguments.as_slice() else {
            continue;
        };
        let Some(table) = ix.symbol(table) else {
            continue;
        };
        let Some(callback) = ix.function_of(callback) else {
            continue;
        };
        if !is_glyph_sample_table(ix, table) {
            continue;
        }
        out.binding(
            ix,
            CANVAS_GLYPH_SAMPLES,
            table,
            "array of [glyph, pixel-coordinate rows] handed to the glyph observation queue",
        );
        out.function(
            ix,
            GLYPH_RENDER_PROBE,
            callback,
            "glyph queue callback that hashes every sampled row in reverse order and completes the observer",
        );
    }
}

// ---------------------------------------------------------------------------
// MAIN bindings
// ---------------------------------------------------------------------------

/// Source-closed array literal of `[string, [[number, number], ...]]` rows.
fn is_glyph_sample_table(ix: &Index<'_>, symbol: SymbolId) -> bool {
    let Some(origin) = ix.single_origin(symbol) else {
        return false;
    };
    let Expression::ArrayExpression(array) = strip(origin) else {
        return false;
    };
    if array.elements.is_empty() {
        return false;
    }
    array.elements.iter().all(|element| {
        let Some(element) = element.as_expression() else {
            return false;
        };
        let Expression::ArrayExpression(row) = strip(element) else {
            return false;
        };
        if row.elements.len() != 2 {
            return false;
        }
        let Some(first) = row.elements[0].as_expression() else {
            return false;
        };
        if string_value(first).is_none() {
            return false;
        }
        let Some(Expression::ArrayExpression(points)) =
            row.elements[1].as_expression().map(strip)
        else {
            return false;
        };
        if points.elements.is_empty() {
            return false;
        }
        points.elements.iter().all(|point| {
            let Some(point) = point.as_expression() else {
                return false;
            };
            let Expression::ArrayExpression(pair) = strip(point) else {
                return false;
            };
            pair.elements.len() == 2
                && pair.elements.iter().all(|element| {
                    element.as_expression().is_some_and(|value| {
                        matches!(strip(value), Expression::NumericLiteral(_))
                    })
                })
        })
    })
}

/// Source-closed array literal of `[string, number, number, number]` rows.
fn is_emoji_probe_table(ix: &Index<'_>, symbol: SymbolId) -> bool {
    let Some(origin) = ix.single_origin(symbol) else {
        return false;
    };
    let Expression::ArrayExpression(array) = strip(origin) else {
        return false;
    };
    if array.elements.is_empty() {
        return false;
    }
    array.elements.iter().all(|element| {
        let Some(element) = element.as_expression() else {
            return false;
        };
        let Expression::ArrayExpression(row) = strip(element) else {
            return false;
        };
        if row.elements.len() != 4 {
            return false;
        }
        let Some(first) = row.elements[0].as_expression() else {
            return false;
        };
        if string_value(first).is_none() {
            return false;
        }
        row.elements[1..].iter().all(|element| {
            element.as_expression().is_some_and(|value| {
                matches!(strip(value), Expression::NumericLiteral(_))
            })
        })
    })
}

fn is_numeric_array(ix: &Index<'_>, symbol: SymbolId) -> bool {
    let Some(origin) = ix.single_origin(symbol) else {
        return false;
    };
    let Expression::ArrayExpression(array) = strip(origin) else {
        return false;
    };
    !array.elements.is_empty()
        && array.elements.iter().all(|element| {
            element.as_expression().is_some_and(|value| {
                matches!(strip(value), Expression::NumericLiteral(_))
            })
        })
}

fn is_string_array(ix: &Index<'_>, symbol: SymbolId) -> bool {
    let Some(origin) = ix.single_origin(symbol) else {
        return false;
    };
    let Expression::ArrayExpression(array) = strip(origin) else {
        return false;
    };
    !array.elements.is_empty()
        && array.elements.iter().all(|element| {
            element.as_expression().is_some_and(|value| string_value(value).is_some())
        })
}

/// An array literal holding host values (identifiers/members/unary/conditional),
/// not a plain literal list.
fn is_host_reference_array(ix: &Index<'_>, symbol: SymbolId) -> bool {
    let Some(origin) = ix.single_origin(symbol) else {
        return false;
    };
    let Expression::ArrayExpression(array) = strip(origin) else {
        return false;
    };
    array.elements.len() >= 5
        && array.elements.iter().all(|element| {
            element.as_expression().is_some_and(|value| {
                !matches!(strip(value), Expression::StringLiteral(_) | Expression::NumericLiteral(_))
            })
        })
}

/// `[].slice.call.bind([].slice)`.
fn is_slice_binder(ix: &Index<'_>, symbol: SymbolId) -> bool {
    let Some(origin) = ix.single_origin(symbol) else {
        return false;
    };
    let Expression::CallExpression(call) = strip(origin) else {
        return false;
    };
    let Some((bound, "bind")) = member_of(&call.callee) else {
        return false;
    };
    let Some((inner, "call")) = member_of(bound) else {
        return false;
    };
    let Some((array, "slice")) = member_of(inner) else {
        return false;
    };
    if !matches!(strip(array), Expression::ArrayExpression(found) if found.elements.is_empty()) {
        return false;
    }
    let arguments = call_arguments(call);
    matches!(arguments.as_slice(), [only] if member_of(only).is_some_and(|(target, name)| {
        name == "slice"
            && matches!(strip(target), Expression::ArrayExpression(found) if found.elements.is_empty())
    }))
}

fn is_hash_wrapper_call(ix: &Index<'_>, symbol: SymbolId) -> bool {
    let Some(origin) = ix.single_origin(symbol) else {
        return false;
    };
    let Expression::CallExpression(call) = strip(origin) else {
        return false;
    };
    let arguments = call_arguments(call);
    matches!(arguments.as_slice(), [only] if ix.function_of(only).is_some())
        && ix.function_of(&call.callee).is_some()
}

fn main_bindings(ix: &Index<'_>, out: &mut Out, proven: &Proven) {
    // The proven consumers, as read sets of their subtrees.
    let mut font_reads = BTreeSet::new();
    if let Some(f) = proven.font_family_metrics {
        ix.reads_under(f, &mut font_reads);
    }
    let mut seed_reads = BTreeSet::new();
    if let Some(f) = proven.seeded_font_probe {
        ix.reads_under(f, &mut seed_reads);
    }
    let mut media_reads = BTreeSet::new();
    if let Some(f) = proven.media_codec_probe {
        ix.reads_under(f, &mut media_reads);
    }
    let mut webgl_reads = BTreeSet::new();
    if let Some(f) = proven.webgl_probe {
        ix.reads_under(f, &mut webgl_reads);
    }
    let mut canvas_reads = BTreeSet::new();
    if let Some(f) = proven.canvas_closure {
        ix.reads_under(f, &mut canvas_reads);
    }
    let mut browser_reads = BTreeSet::new();
    if let Some(f) = proven.browser_closure {
        ix.reads_under(f, &mut browser_reads);
    }

    let captured = |symbol: SymbolId, consumer: Option<usize>| {
        consumer.is_some_and(|f| ix.declared_outside(symbol, f))
    };

    for symbol in ix.values.keys().copied().collect::<Vec<_>>() {
        if is_glyph_sample_table(ix, symbol)
            && browser_reads.contains(&symbol)
            && captured(symbol, proven.browser_closure)
        {
            out.binding(
                ix,
                CANVAS_GLYPH_SAMPLES,
                symbol,
                "closed [glyph, pixel-row] table consumed by the browser-fingerprint glyph queue",
            );
        }
        if is_emoji_probe_table(ix, symbol)
            && browser_reads.contains(&symbol)
            && captured(symbol, proven.browser_closure)
        {
            out.binding(
                ix,
                EMOJI_RENDER_PROBES,
                symbol,
                "closed [glyph, x, y, channel] table consumed by the emoji sampling queue",
            );
        }
        if is_numeric_array(ix, symbol)
            && font_reads.contains(&symbol)
            && captured(symbol, proven.font_family_metrics)
        {
            out.binding(
                ix,
                FONT_PROBE_CODE_POINTS,
                symbol,
                "numeric code-point list iterated by the font-family metric observable",
            );
        }
        if is_string_array(ix, symbol)
            && seed_reads.contains(&symbol)
            && captured(symbol, proven.seeded_font_probe)
        {
            out.binding(
                ix,
                FONT_PROBE_FAMILIES,
                symbol,
                "font-family name list mapped to one metric observable per family",
            );
        }
        if is_string_array(ix, symbol)
            && media_reads.contains(&symbol)
            && captured(symbol, proven.media_codec_probe)
        {
            out.binding(
                ix,
                MEDIA_CODEC_PROBE_STRINGS,
                symbol,
                "media type/codec string list asked of the video element's canPlayType",
            );
        }
        if is_host_reference_array(ix, symbol)
            && webgl_reads.contains(&symbol)
            && captured(symbol, proven.webgl_probe)
        {
            out.binding(
                ix,
                NATIVE_CODE_CHECK_TARGETS,
                symbol,
                "host callables and globals filtered by the WebGL probe to test native source text",
            );
        }
        if is_slice_binder(ix, symbol)
            && canvas_reads.contains(&symbol)
            && captured(symbol, proven.canvas_closure)
        {
            out.binding(
                ix,
                SLICE_ARRAY_LIKE,
                symbol,
                "bound Array.prototype.slice used to cut pixel buffers by byte range",
            );
        }
        if is_hash_wrapper_call(ix, symbol)
            && seed_reads.contains(&symbol)
            && captured(symbol, proven.seeded_font_probe)
        {
            out.binding(
                ix,
                HASH_OBSERVABLE_BINDER,
                symbol,
                "call result binding a hash function into an observable mapper used by the font-metric probe",
            );
        }
        if let Some(origin) = ix.single_origin(symbol) {
            if ix.path(origin).as_deref() == Some("String.fromCharCode")
                && font_reads.contains(&symbol)
                && captured(symbol, proven.font_family_metrics)
            {
                out.binding(
                    ix,
                    STRING_FROM_CHAR_CODE,
                    symbol,
                    "String.fromCharCode read by the font-metric reducer to build the measured glyph",
                );
            }
        }
    }

    // The single MAIN aliases of the observable/queue helpers that the canvas
    // probes call. Their holder roles belong to p02/p10/p03; these aliases were
    // not named by those packages.
    for symbol in ix.values.keys().copied().collect::<Vec<_>>() {
        let Some(origin) = ix.single_origin(symbol) else {
            continue;
        };
        if ix.symbol(origin).is_none() || ix.function_of(origin).is_none() {
            continue;
        }
        if fork_join_alias(ix, symbol, proven) {
            out.binding(
                ix,
                FORK_JOIN_ALIAS,
                symbol,
                "single MAIN alias of the observable-joining factory the seeded font and canvas probes subscribe",
            );
        }
        if reduce_alias(ix, symbol, proven) {
            out.binding(
                ix,
                REDUCE_OPERATOR_ALIAS,
                symbol,
                "single MAIN alias of the reduce operator piped over the font code-point observable",
            );
        }
        if aspect_ratio_alias(ix, symbol, proven) {
            out.binding(
                ix,
                ASPECT_RATIO_ALIAS,
                symbol,
                "single MAIN alias of the media-feature prober called by the browser-fingerprint closure",
            );
        }    }
}

/// The alias is called with one non-function argument and the result is
/// subscribed by one of the proven probes.
fn fork_join_alias(ix: &Index<'_>, symbol: SymbolId, proven: &Proven) -> bool {
    let consumers = [
        proven.seeded_font_probe,
        proven.canvas_closure,
        proven.browser_closure,
    ]
    .into_iter()
    .flatten()
    .filter(|f| ix.declared_outside(symbol, *f))
    .collect::<Vec<_>>();
    if consumers.is_empty() {
        return false;
    }
    let mut readers = BTreeSet::new();
    for f in &consumers {
        ix.reads_under(*f, &mut readers);
    }
    if !readers.contains(&symbol) {
        return false;
    }
    for call in &ix.all_calls {
        if ix.symbol(&call.callee) != Some(symbol) {
            continue;
        }
        let arguments = call_arguments(call);
        let [only] = arguments.as_slice() else {
            continue;
        };
        if ix.function_of(only).is_some() {
            continue;
        }
        // The result is subscribed: a local holds the call and `.subscribe` is
        // called on that local.
        if ix.values.iter().any(|(receiver, values)| {
            values.iter().any(|value| value.span() == call.span)
                && ix.own.iter().any(|own| {
                    own.calls.iter().any(|subscriber| {
                        method_call(subscriber).is_some_and(|method| {
                            method.name == "subscribe"
                                && ix.symbol(method.receiver) == Some(*receiver)
                        })
                    })
                })
        }) {
            return true;
        }
    }
    false
}

/// The alias is called with `(reducer, seed-string)` inside the font-family
/// metric callback.
fn reduce_alias(ix: &Index<'_>, symbol: SymbolId, proven: &Proven) -> bool {
    let Some(family) = proven.font_family_metrics else {
        return false;
    };
    if !ix.declared_outside(symbol, family) {
        return false;
    }
    ix.subtree(family).iter().any(|f| {
        ix.own[*f].calls.iter().any(|call| {
            if ix.symbol(&call.callee) != Some(symbol) {
                return false;
            }
            let arguments = call_arguments(call);
            matches!(arguments.as_slice(), [reducer, seed]
                if ix.function_of(reducer).is_some() && string_value(seed).is_some())
        })
    })
}

/// The alias is called with `(global, feature, low, high, onResolve, onReject)`
/// by the browser-fingerprint closure.
fn aspect_ratio_alias(ix: &Index<'_>, symbol: SymbolId, proven: &Proven) -> bool {
    let Some(closure) = proven.browser_closure else {
        return false;
    };
    if !ix.declared_outside(symbol, closure) {
        return false;
    }
    ix.subtree(closure).iter().any(|f| {
        ix.own[*f].calls.iter().any(|call| {
            if ix.symbol(&call.callee) != Some(symbol) {
                return false;
            }
            let arguments = call_arguments(call);
            arguments.len() == 6
                && string_value(arguments[1]).is_some()
                && ix.function_of(arguments[4]).is_some()
                && ix.function_of(arguments[5]).is_some()
        })
    })
}
