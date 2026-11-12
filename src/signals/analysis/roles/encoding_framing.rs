//! Package `p05_encoding_framing` finders. See `roles/mod.rs` for the rules.
//!
//! Local writers, keystream and pool helpers, bundle framing and its MAIN
//! state. Every predicate reads a function's *own* body (nested functions are
//! separate units), its formal slots (a named formal and `arguments[N]` are the
//! same slot, through [`ParameterSlots`]), the functions it returns, and the
//! MAIN bindings it reads. Member names read alike as `a.b` and `a["b"]`.
//!
//! The roles `semantic_names` already proves (the NUMBER writer, the string
//! intern-writer factory, the PRNG byte-source factory, the hashers and their
//! dispatcher, the stream-cipher factory and its MAIN result, the error ring
//! and the empty-signal sentinel) are never claimed here; their *inner*
//! returned functions and the MAIN state around them are. Cipher helpers are
//! opaque: a name says what the function does with its input, never which
//! standard algorithm it is.

use std::collections::{BTreeMap, BTreeSet};

use oxc_ast::ast::*;
use oxc_ast::AstKind;
use oxc_semantic::{NodeId, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::operator::{
    AssignmentOperator, BinaryOperator, LogicalOperator, UnaryOperator, UpdateOperator,
};

use crate::signals::extract::bindings::{array_elements, object_members, object_members_of, ParameterSlots};
use crate::signals::schema::Confidence;
use super::{FinderClaim, FinderCtx, FinderRole, FinderRoleKind};

const fn function(role: &'static str, name: &'static str) -> FinderRole {
    FinderRole { role, name, kind: FinderRoleKind::Function }
}

const fn alias(role: &'static str, name: &'static str) -> FinderRole {
    FinderRole { role, name, kind: FinderRoleKind::FunctionAlias }
}

const fn main_binding(role: &'static str, name: &'static str) -> FinderRole {
    FinderRole { role, name, kind: FinderRoleKind::MainBinding }
}

const fn class(role: &'static str, name: &'static str) -> FinderRole {
    FinderRole { role, name, kind: FinderRoleKind::Class }
}

/// Every role this package can claim, with the one name it receives.
pub(crate) const VOCABULARY: &[FinderRole] = &[
    // Random-fraction pool.
    function("random_fraction_pool_draw", "drawRandomFractionFromPool"),
    alias("random_fraction_pool_draw_alias", "randomFractionPoolDraw"),
    main_binding("random_fraction_pool_refill_alias", "refillRandomFractionPool"),
    // Character codes.
    function("first_char_code", "firstCharCode"),
    alias("first_char_code_alias", "firstCharCodeReader"),
    main_binding("char_code_unit_reader", "charCodeUnitReader"),
    // Typed-array feature state.
    main_binding("typed_array_hash_supported", "typedArrayHashSupported"),
    main_binding("uint8_array_constructor", "uint8ArrayConstructor"),
    main_binding("float64_scratch_buffer", "float64ScratchBuffer"),
    main_binding("nan_float64_bytes_fallback", "nanFloat64BytesFallback"),
    main_binding("batch_scheduler_alias", "BatchSchedulerAlias"),
    // Functions returned by the proven factories.
    function("interned_string_writer", "writeInternedString"),
    function("keystream_next_byte", "nextKeystreamByte"),
    function("stream_cipher_apply", "applyStreamCipher"),
    // Error ring.
    function("signal_error_enqueue", "enqueueSignalError"),
    class("ring_buffer_class", "RingBuffer"),
    function("ring_buffer_constructor", "RingBufferConstructor"),
    function("ring_buffer_put", "putRingBufferEntry"),
    function("ring_buffer_to_array", "ringBufferSnapshot"),
    function("ring_buffer_clear", "clearRingBuffer"),
    // Emoji glyph pixel probe.
    function("emoji_glyph_pixel_collector", "collectEmojiGlyphPixels"),
    main_binding("emoji_glyph_pixel_probes", "emojiGlyphPixelProbes"),
    // Bundle framing.
    function("signal_bundle_encoder_factory", "createSignalBundleEncoder"),
    alias("signal_bundle_encoder_factory_alias", "signalBundleEncoderFactory"),
    function("signal_bundle_encoder", "encodeSignalBundle"),
    main_binding("bundle_value_encoder", "binaryDataEncoder"),
    main_binding("bundle_entropy_counter", "entropyCounter"),
    main_binding("bundle_sentinel_extension", "emptySignalSentinelExtension"),
    main_binding("bundle_decode_only_extension", "decodeOnlyExtension"),
];

/// Structural claims of this package.
pub(crate) fn find(ctx: &FinderCtx) -> Vec<FinderClaim> {
    let index = Index::new(ctx);
    let mut out = Out::default();
    index.pool_draw(&mut out);
    index.first_char_code(&mut out);
    index.main_feature_state(&mut out);
    index.interned_string_writer(&mut out);
    index.keystream_next_byte(&mut out);
    index.stream_cipher_apply(&mut out);
    index.signal_error_enqueue(&mut out);
    index.ring_buffers(&mut out);
    index.emoji_probe(&mut out);
    index.bundle_encoder(&mut out);
    index.batch_scheduler_alias(&mut out);
    out.claims
}

// ===========================================================================
// Output
// ===========================================================================

#[derive(Default)]
struct Out {
    claims: Vec<FinderClaim>,
    seen: BTreeSet<(&'static str, u32, u32)>,
}

impl Out {
    fn function(
        &mut self,
        role: &'static str,
        span: Span,
        alias_role: Option<&'static str>,
        evidence: Vec<String>,
    ) {
        if self.seen.insert((role, span.start, span.end)) {
            self.claims.push(FinderClaim::Function {
                role,
                function_span: span,
                alias_role,
                confidence: Confidence::High,
                evidence,
            });
        }
    }

    fn main(&mut self, role: &'static str, identifier_span: Span, evidence: Vec<String>) {
        if self.seen.insert((role, identifier_span.start, identifier_span.end)) {
            self.claims.push(FinderClaim::MainBinding {
                role,
                identifier_span,
                confidence: Confidence::High,
                evidence,
            });
        }
    }

    fn class(&mut self, role: &'static str, class_span: Span, evidence: Vec<String>) {
        if self.seen.insert((role, class_span.start, class_span.end)) {
            self.claims.push(FinderClaim::ClassBinding {
                role,
                class_span,
                confidence: Confidence::High,
                evidence,
            });
        }
    }
}

// ===========================================================================
// Index
// ===========================================================================

const NONE: u32 = u32::MAX;

struct FnEntry<'a> {
    node: NodeId,
    span: Span,
    params: &'a FormalParameters<'a>,
    /// Declared or own name of a `function`, when it has one.
    symbol: Option<SymbolId>,
    /// An arrow whose body is a bare expression.
    expression_body: bool,
    nested: Vec<usize>,
    own: Vec<NodeId>,
}

/// One write of a binding.
#[derive(Clone, Copy)]
enum Write<'a> {
    /// `let x = value` or `x = value`.
    Value(&'a Expression<'a>, Span),
    /// `++x`, `x++`.
    Increment,
    /// Anything else (compound assignment, decrement).
    Other,
}

struct Index<'c, 'a> {
    ctx: &'c FinderCtx<'c, 'a>,
    fns: Vec<FnEntry<'a>>,
    by_span: BTreeMap<(u32, u32), usize>,
    by_symbol: BTreeMap<SymbolId, usize>,
    writes: BTreeMap<SymbolId, Vec<Write<'a>>>,
    /// `Ctor.prototype.name = fn` methods, by constructor symbol.
    prototype_methods: BTreeMap<SymbolId, BTreeMap<String, usize>>,
    slots: ParameterSlots<'a, 'a>,
}

impl<'c, 'a> Index<'c, 'a> {
    fn new(ctx: &'c FinderCtx<'c, 'a>) -> Self {
        let nodes = ctx.nodes;
        let count = nodes.len();
        let mut index = Self {
            ctx,
            fns: Vec::new(),
            by_span: BTreeMap::new(),
            by_symbol: BTreeMap::new(),
            writes: BTreeMap::new(),
            prototype_methods: BTreeMap::new(),
            slots: ParameterSlots::new(ctx.program, ctx.scoping, ctx.nodes),
        };
        let mut owners = vec![NONE; count];
        let mut fn_of_node = vec![NONE; count];
        for (id, node) in nodes.iter_enumerated() {
            let position = id.index();
            let parent = nodes.parent_id(id);
            let owner = if parent == id || parent.index() >= position {
                NONE
            } else if fn_of_node[parent.index()] != NONE {
                fn_of_node[parent.index()]
            } else {
                owners[parent.index()]
            };
            owners[position] = owner;
            let entry = match node.kind() {
                AstKind::Function(function) => Some((
                    function.span,
                    &*function.params,
                    function.id.as_ref().and_then(|id| id.symbol_id.get()),
                    false,
                )),
                AstKind::ArrowFunctionExpression(arrow) => {
                    Some((arrow.span, &*arrow.params, None, arrow.expression))
                }
                _ => None,
            };
            if let Some((span, params, symbol, expression_body)) = entry {
                let function_index = index.fns.len();
                index.fns.push(FnEntry {
                    node: id,
                    span,
                    params,
                    symbol,
                    expression_body,
                    nested: Vec::new(),
                    own: Vec::new(),
                });
                index.by_span.insert((span.start, span.end), function_index);
                if let Some(symbol) = symbol {
                    index.by_symbol.insert(symbol, function_index);
                }
                fn_of_node[position] = function_index as u32;
                if owner != NONE {
                    index.fns[owner as usize].nested.push(function_index);
                }
                continue;
            }
            if owner != NONE {
                index.fns[owner as usize].own.push(id);
            }
            index.record_write(node.kind());
        }
        index.collect_prototype_methods();
        index
    }

    fn record_write(&mut self, kind: AstKind<'a>) {
        let scoping = self.ctx.scoping;
        let (symbol, write) = match kind {
            AstKind::VariableDeclarator(declarator) => {
                let BindingPattern::BindingIdentifier(identifier) = &declarator.id else {
                    return;
                };
                let Some(init) = &declarator.init else {
                    return;
                };
                (identifier.symbol_id.get(), Write::Value(init, declarator.span))
            }
            AstKind::AssignmentExpression(assignment) => {
                let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left
                else {
                    return;
                };
                let write = if assignment.operator == AssignmentOperator::Assign {
                    Write::Value(&assignment.right, assignment.span)
                } else {
                    Write::Other
                };
                (crate::oxc::identifier_symbol(identifier, scoping), write)
            }
            AstKind::UpdateExpression(update) => {
                let SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier) =
                    &update.argument
                else {
                    return;
                };
                let write = if update.operator == UpdateOperator::Increment {
                    Write::Increment
                } else {
                    Write::Other
                };
                (crate::oxc::identifier_symbol(identifier, scoping), write)
            }
            _ => return,
        };
        if let Some(symbol) = symbol {
            self.writes.entry(symbol).or_default().push(write);
        }
    }

    fn collect_prototype_methods(&mut self) {
        let mut methods: BTreeMap<SymbolId, BTreeMap<String, usize>> = BTreeMap::new();
        for node in self.ctx.nodes.iter() {
            let AstKind::AssignmentExpression(assignment) = node.kind() else {
                continue;
            };
            if assignment.operator != AssignmentOperator::Assign {
                continue;
            }
            let Some((object, name)) = target_member(&assignment.left) else {
                continue;
            };
            if member_name(object) != Some("prototype") {
                continue;
            }
            let Some(constructor) = member_object(object).and_then(|ctor| self.symbol(ctor)) else {
                continue;
            };
            if let Some(method) = self.function_value(&assignment.right) {
                methods.entry(constructor).or_default().insert(name.to_string(), method);
            }
        }
        self.prototype_methods = methods;
    }

    // -----------------------------------------------------------------------
    // Small readers
    // -----------------------------------------------------------------------

    fn kinds(&self, function: usize) -> impl Iterator<Item = AstKind<'a>> + '_ {
        self.fns[function].own.iter().map(move |id| self.ctx.nodes.kind(*id))
    }

    fn symbol(&self, expression: &Expression<'_>) -> Option<SymbolId> {
        crate::oxc::expression_symbol(expression.without_parentheses(), self.ctx.scoping)
    }

    /// The slot `expression` denotes in `function` (formal or `arguments[N]`).
    fn slot(&self, expression: &Expression<'_>, function: usize) -> Option<u32> {
        let slot = self.slots.parameter_slot(expression.without_parentheses())?;
        (slot.owner == self.fns[function].node).then_some(slot.index)
    }

    /// Slot `index` of `function`, read directly or through a local whose
    /// single origin is that slot.
    fn is_slot(&self, expression: &Expression<'_>, function: usize, index: u32) -> bool {
        self.slot(expression, function) == Some(index)
            || self
                .symbol(expression)
                .and_then(|symbol| self.single_value(symbol))
                .is_some_and(|value| self.slot(value, function) == Some(index))
    }

    /// The single plain value ever written to `symbol`.
    fn single_value(&self, symbol: SymbolId) -> Option<&'a Expression<'a>> {
        match self.writes.get(&symbol)?.as_slice() {
            [Write::Value(value, _)] => Some(value),
            _ => None,
        }
    }

    fn declared_outside(&self, symbol: SymbolId, function: usize) -> bool {
        !contains(self.fns[function].span, self.ctx.scoping.symbol_span(symbol))
    }

    fn is_global(&self, expression: &Expression<'_>, name: &str) -> bool {
        matches!(expression.without_parentheses(), Expression::Identifier(identifier)
            if identifier.name == name
                && crate::oxc::identifier_symbol(identifier, self.ctx.scoping).is_none())
    }

    fn label(&self, span: Span) -> String {
        let text = self.ctx.source.get(span.start as usize..span.end as usize).unwrap_or("");
        let line = text.lines().next().unwrap_or("").trim();
        let mut shown: String = line.chars().take(90).collect();
        if shown.len() < line.len() || text.lines().nth(1).is_some() {
            shown.push_str(" ...");
        }
        shown
    }

    /// The declaring identifier span of a binding.
    fn identifier_span(&self, symbol: SymbolId) -> Span {
        self.ctx.scoping.symbol_span(symbol)
    }

    /// The function an expression denotes: inline, or a named function.
    fn function_value(&self, expression: &Expression<'_>) -> Option<usize> {
        let span = expression.without_parentheses().span();
        if let Some(index) = self.by_span.get(&(span.start, span.end)) {
            return Some(*index);
        }
        self.by_symbol.get(&self.symbol(expression)?).copied()
    }

    fn method_function(&self, method: &MethodDefinition<'a>) -> Option<usize> {
        let span = method.value.span;
        self.by_span.get(&(span.start, span.end)).copied()
    }

    /// Nested functions of `function` that it returns (by name or inline).
    fn returned_functions(&self, function: usize) -> Vec<usize> {
        let mut out = Vec::new();
        for argument in self.returns(function) {
            if let Some(returned) = self.function_value(argument) {
                if self.fns[function].nested.contains(&returned) && !out.contains(&returned) {
                    out.push(returned);
                }
            }
        }
        out
    }

    fn returns(&self, function: usize) -> impl Iterator<Item = &'a Expression<'a>> + '_ {
        self.kinds(function).filter_map(|kind| match kind {
            AstKind::ReturnStatement(statement) => statement.argument.as_ref(),
            _ => None,
        })
    }

    fn calls(&self, function: usize) -> impl Iterator<Item = &'a CallExpression<'a>> + '_ {
        self.kinds(function).filter_map(|kind| match kind {
            AstKind::CallExpression(call) => Some(call),
            _ => None,
        })
    }

    fn method_calls<'n>(
        &'n self,
        function: usize,
        method: &'n str,
    ) -> impl Iterator<Item = &'a CallExpression<'a>> + 'n {
        self.calls(function).filter(move |call| member_name(&call.callee) == Some(method))
    }

    fn binaries(&self, function: usize) -> impl Iterator<Item = &'a BinaryExpression<'a>> + '_ {
        self.kinds(function).filter_map(|kind| match kind {
            AstKind::BinaryExpression(binary) => Some(binary),
            _ => None,
        })
    }

    fn count_binary(&self, function: usize, operator: BinaryOperator) -> usize {
        self.binaries(function).filter(|binary| binary.operator == operator).count()
    }

    fn has_loop(&self, function: usize) -> bool {
        self.kinds(function).any(|kind| {
            matches!(
                kind,
                AstKind::WhileStatement(_) | AstKind::DoWhileStatement(_) | AstKind::ForStatement(_)
            )
        })
    }

    /// `(symbol, value)` of every plain local or captured write in the
    /// function's own body.
    fn own_values(&self, function: usize) -> Vec<(SymbolId, &'a Expression<'a>)> {
        let mut out = Vec::new();
        for kind in self.kinds(function) {
            match kind {
                AstKind::VariableDeclarator(declarator) => {
                    if let (BindingPattern::BindingIdentifier(identifier), Some(init)) =
                        (&declarator.id, &declarator.init)
                    {
                        if let Some(symbol) = identifier.symbol_id.get() {
                            out.push((symbol, init));
                        }
                    }
                }
                AstKind::AssignmentExpression(assignment)
                    if assignment.operator == AssignmentOperator::Assign =>
                {
                    if let AssignmentTarget::AssignmentTargetIdentifier(identifier) =
                        &assignment.left
                    {
                        if let Some(symbol) =
                            crate::oxc::identifier_symbol(identifier, self.ctx.scoping)
                        {
                            out.push((symbol, &assignment.right));
                        }
                    }
                }
                _ => {}
            }
        }
        out
    }

    /// Member names read anywhere in the function's own body.
    fn member_names(&self, function: usize) -> BTreeSet<&'a str> {
        self.kinds(function).filter_map(|kind| kind_member(kind).map(|(_, name)| name)).collect()
    }

    /// Every resolved binding declared outside `function` its own body reads.
    fn captured_reads(&self, function: usize) -> BTreeSet<SymbolId> {
        let mut out = BTreeSet::new();
        for kind in self.kinds(function) {
            if let AstKind::IdentifierReference(identifier) = kind {
                if let Some(symbol) = crate::oxc::identifier_symbol(identifier, self.ctx.scoping) {
                    if self.declared_outside(symbol, function) {
                        out.insert(symbol);
                    }
                }
            }
        }
        out
    }

    /// How many argument slots the function reads (formals or `arguments[N]`).
    fn slot_width(&self, function: usize) -> usize {
        let mut width = self.fns[function].params.items.len();
        for kind in self.kinds(function) {
            if let AstKind::ComputedMemberExpression(member) = kind {
                if let (Expression::Identifier(object), Some(index)) =
                    (&member.object, numeric_literal(&member.expression))
                {
                    if object.name == "arguments"
                        && crate::oxc::identifier_symbol(object, self.ctx.scoping).is_none()
                    {
                        width = width.max(index as usize + 1);
                    }
                }
            }
        }
        width
    }

    // -----------------------------------------------------------------------
    // Random-fraction pool
    // -----------------------------------------------------------------------

    /// `do { if (pool.length < k) refill(pool); x = pool.pop(); } while
    /// (!(x > 0 && x < 1)); return x;` over slot 0.
    fn pool_draw(&self, out: &mut Out) {
        for function in 0..self.fns.len() {
            let pops: Vec<_> = self
                .method_calls(function, "pop")
                .filter(|call| {
                    call.arguments.is_empty()
                        && member_object(&call.callee)
                            .is_some_and(|object| self.is_slot(object, function, 0))
                })
                .collect();
            if pops.is_empty() {
                continue;
            }
            let popped: BTreeSet<SymbolId> = self
                .own_values(function)
                .into_iter()
                .filter(|(_, value)| pops.iter().any(|call| call.span == value.span()))
                .map(|(symbol, _)| symbol)
                .collect();
            let [value] = popped.into_iter().collect::<Vec<_>>()[..] else {
                continue;
            };
            let compares = |operator: BinaryOperator, bound: f64| {
                self.binaries(function).any(|binary| {
                    binary.operator == operator
                        && self.symbol(&binary.left) == Some(value)
                        && numeric_literal(&binary.right) == Some(bound)
                })
            };
            let loops_on_value = self.kinds(function).any(|kind| match kind {
                AstKind::DoWhileStatement(statement) => {
                    self.mentions_symbol(&statement.test, value)
                }
                AstKind::WhileStatement(statement) => self.mentions_symbol(&statement.test, value),
                _ => false,
            });
            let returns_value =
                self.returns(function).any(|argument| self.symbol(argument) == Some(value));
            let length_guard = self.binaries(function).any(|binary| {
                binary.operator == BinaryOperator::LessThan
                    && member_name(&binary.left) == Some("length")
                    && member_object(&binary.left)
                        .is_some_and(|object| self.is_slot(object, function, 0))
            });
            let refills: Vec<&CallExpression> = self
                .calls(function)
                .filter(|call| {
                    matches!(call.callee.without_parentheses(), Expression::Identifier(_))
                        && call.arguments.len() == 1
                        && call.arguments[0]
                            .as_expression()
                            .is_some_and(|argument| self.is_slot(argument, function, 0))
                })
                .collect();
            if !(compares(BinaryOperator::GreaterThan, 0.0)
                && compares(BinaryOperator::LessThan, 1.0)
                && loops_on_value
                && returns_value
                && length_guard
                && !refills.is_empty())
            {
                continue;
            }
            out.function(
                "random_fraction_pool_draw",
                self.fns[function].span,
                Some("random_fraction_pool_draw_alias"),
                vec![
                    format!(
                        "pops slot 0 (`{}`) until the value is strictly between 0 and 1, and returns it",
                        self.label(pops[0].span)
                    ),
                    format!(
                        "refills slot 0 through `{}` when its length falls below the guard",
                        self.label(refills[0].span)
                    ),
                ],
            );
            // The refill callee, when it is a MAIN alias of a function. A
            // function declaration belongs to its own package.
            let callees: BTreeSet<SymbolId> =
                refills.iter().filter_map(|call| self.symbol(&call.callee)).collect();
            if let [callee] = callees.into_iter().collect::<Vec<_>>()[..] {
                let aliases_function = self
                    .single_value(callee)
                    .and_then(|value| self.symbol(value))
                    .is_some_and(|target| self.by_symbol.contains_key(&target));
                if aliases_function
                    && !self.by_symbol.contains_key(&callee)
                    && self.declared_outside(callee, function)
                {
                    out.main(
                        "random_fraction_pool_refill_alias",
                        self.identifier_span(callee),
                        vec![format!(
                            "alias of a function; called with the pool by the random-fraction draw when the pool runs low (`{}`)",
                            self.label(refills[0].span)
                        )],
                    );
                }
            }
        }
    }

    fn mentions_symbol(&self, expression: &Expression<'_>, symbol: SymbolId) -> bool {
        let mut found = false;
        visit_expressions(expression, &mut |inner| found |= self.symbol(inner) == Some(symbol));
        found
    }

    fn mentions_global(&self, expression: &Expression<'_>, name: &str) -> bool {
        let mut found = false;
        visit_expressions(expression, &mut |inner| found |= self.is_global(inner, name));
        found
    }

    fn mentions_slot(&self, expression: &Expression<'_>, function: usize, index: u32) -> bool {
        let mut found = false;
        visit_expressions(expression, &mut |inner| {
            found |= self.slot(inner, function) == Some(index);
        });
        found
    }

    // -----------------------------------------------------------------------
    // Character codes
    // -----------------------------------------------------------------------

    /// `s => s.charCodeAt()`: the whole body yields the slot-0 code unit at
    /// index 0.
    fn first_char_code(&self, out: &mut Out) {
        for function in 0..self.fns.len() {
            if self.slot_width(function) != 1 || !self.fns[function].nested.is_empty() {
                continue;
            }
            let Some(body) = self.kinds(function).find_map(|kind| match kind {
                AstKind::FunctionBody(body) => Some(body),
                _ => None,
            }) else {
                continue;
            };
            let [statement] = body.statements.as_slice() else {
                continue;
            };
            let value = match statement {
                Statement::ReturnStatement(statement) => statement.argument.as_ref(),
                Statement::ExpressionStatement(statement)
                    if self.fns[function].expression_body =>
                {
                    Some(&statement.expression)
                }
                _ => None,
            };
            let Some(Expression::CallExpression(call)) =
                value.map(Expression::without_parentheses)
            else {
                continue;
            };
            let index_zero = match call.arguments.as_slice() {
                [] => true,
                [argument] => argument.as_expression().and_then(numeric_literal) == Some(0.0),
                _ => false,
            };
            let on_slot = member_object(&call.callee)
                .is_some_and(|object| self.slot(object, function) == Some(0));
            if member_name(&call.callee) != Some("charCodeAt") || !index_zero || !on_slot {
                continue;
            }
            out.function(
                "first_char_code",
                self.fns[function].span,
                Some("first_char_code_alias"),
                vec![format!(
                    "the whole body returns `{}`: the UTF-16 code unit at index 0 of slot 0",
                    self.label(call.span)
                )],
            );
        }
    }

    // -----------------------------------------------------------------------
    // MAIN feature state of the writers and hashers
    // -----------------------------------------------------------------------

    fn main_feature_state(&self, out: &mut Out) {
        let mut float_scratch = BTreeSet::new();
        for (&symbol, writes) in &self.writes {
            let [Write::Value(value, _)] = writes.as_slice() else {
                continue;
            };
            let value = value.without_parentheses();
            match value {
                // `Function.prototype.call.bind(String.prototype.charCodeAt)`.
                Expression::CallExpression(call) => {
                    let bound = call.arguments.first().and_then(Argument::as_expression);
                    if call.arguments.len() == 1
                        && member_name(&call.callee) == Some("bind")
                        && member_object(&call.callee).and_then(member_name) == Some("call")
                        && bound.and_then(member_name) == Some("charCodeAt")
                    {
                        out.main(
                            "char_code_unit_reader",
                            self.identifier_span(symbol),
                            vec![format!(
                                "unbound `charCodeAt` taken through `call.bind`: `{}`",
                                self.label(call.span)
                            )],
                        );
                    }
                }
                // `typeof ArrayBuffer.isView === "function" && typeof Int32Array ...`.
                Expression::LogicalExpression(_) => {
                    let mut leaves = Vec::new();
                    and_leaves(value, &mut leaves);
                    let subjects: BTreeSet<String> =
                        leaves.iter().filter_map(|leaf| self.typeof_subject(leaf)).collect();
                    if leaves.iter().all(|leaf| self.is_feature_test(leaf))
                        && subjects.contains("ArrayBuffer.isView")
                        && subjects.contains("Int32Array")
                    {
                        out.main(
                            "typed_array_hash_supported",
                            self.identifier_span(symbol),
                            vec![format!(
                                "feature conjunction over ArrayBuffer.isView and Int32Array: `{}`",
                                self.label(value.span())
                            )],
                        );
                    }
                }
                Expression::ConditionalExpression(conditional) => {
                    let null_otherwise = matches!(
                        conditional.alternate.without_parentheses(),
                        Expression::NullLiteral(_)
                    );
                    // `typeof Uint8Array === "function" ? Uint8Array : null`.
                    if null_otherwise
                        && self.typeof_subject(&conditional.test).as_deref() == Some("Uint8Array")
                        && self.is_global(&conditional.consequent, "Uint8Array")
                    {
                        out.main(
                            "uint8_array_constructor",
                            self.identifier_span(symbol),
                            vec![format!(
                                "the host Uint8Array constructor when it exists, else null: `{}`",
                                self.label(value.span())
                            )],
                        );
                    }
                    // `... ? new Float64Array(1) : null`.
                    if let Expression::NewExpression(new) =
                        conditional.consequent.without_parentheses()
                    {
                        let one = matches!(new.arguments.as_slice(), [argument]
                            if argument.as_expression().and_then(numeric_literal) == Some(1.0));
                        if null_otherwise && one && self.is_global(&new.callee, "Float64Array") {
                            float_scratch.insert(symbol);
                            out.main(
                                "float64_scratch_buffer",
                                self.identifier_span(symbol),
                                vec![format!(
                                    "one-element Float64Array scratch cell, null when unsupported: `{}`",
                                    self.label(value.span())
                                )],
                            );
                        }
                    }
                }
                _ => {}
            }
        }
        self.nan_fallback(out, &float_scratch);
    }

    /// `let u; if (scratch) u = null; else { u = [..]; ... } fallback = u;`:
    /// the alias of the byte array used when no float scratch cell exists.
    fn nan_fallback(&self, out: &mut Out, float_scratch: &BTreeSet<SymbolId>) {
        if float_scratch.is_empty() {
            return;
        }
        for (&symbol, writes) in &self.writes {
            let [Write::Value(value, _)] = writes.as_slice() else {
                continue;
            };
            let Some(holder) = self.symbol(value) else {
                continue;
            };
            let Some([first, second]) = self.writes.get(&holder).map(Vec::as_slice) else {
                continue;
            };
            let (null_write, array_write) = match (first, second) {
                (Write::Value(left, null), Write::Value(right, array))
                    if matches!(left, Expression::NullLiteral(_))
                        && matches!(right, Expression::ArrayExpression(_)) =>
                {
                    (*null, *array)
                }
                (Write::Value(left, array), Write::Value(right, null))
                    if matches!(left, Expression::ArrayExpression(_))
                        && matches!(right, Expression::NullLiteral(_)) =>
                {
                    (*null, *array)
                }
                _ => continue,
            };
            let gated = self.ctx.nodes.iter().any(|node| {
                let AstKind::IfStatement(statement) = node.kind() else {
                    return false;
                };
                let Some(alternate) = &statement.alternate else {
                    return false;
                };
                self.symbol(&statement.test).is_some_and(|test| float_scratch.contains(&test))
                    && contains(statement.consequent.span(), null_write)
                    && contains(alternate.span(), array_write)
            });
            if gated {
                out.main(
                    "nan_float64_bytes_fallback",
                    self.identifier_span(symbol),
                    vec![format!(
                        "alias of the byte array written only when the Float64Array scratch cell is missing (`{}`)",
                        self.label(array_write)
                    )],
                );
            }
        }
    }

    /// The subject of a `typeof X ...` / `typeof X.y ...` test on a global.
    fn typeof_subject(&self, test: &Expression<'_>) -> Option<String> {
        let Expression::BinaryExpression(binary) = test.without_parentheses() else {
            return None;
        };
        let Expression::UnaryExpression(unary) = binary.left.without_parentheses() else {
            return None;
        };
        if unary.operator != UnaryOperator::Typeof {
            return None;
        }
        self.global_path(&unary.argument)
    }

    fn global_path(&self, expression: &Expression<'_>) -> Option<String> {
        match expression.without_parentheses() {
            Expression::Identifier(identifier)
                if crate::oxc::identifier_symbol(identifier, self.ctx.scoping).is_none() =>
            {
                Some(identifier.name.to_string())
            }
            other => {
                let name = member_name(other)?;
                let object = self.global_path(member_object(other)?)?;
                Some(format!("{object}.{name}"))
            }
        }
    }

    fn is_feature_test(&self, leaf: &Expression<'_>) -> bool {
        let Expression::BinaryExpression(binary) = leaf.without_parentheses() else {
            return false;
        };
        self.typeof_subject(leaf).is_some()
            || (matches!(binary.right.without_parentheses(), Expression::NullLiteral(_))
                && self.global_path(&binary.left).is_some())
    }

    // -----------------------------------------------------------------------
    // Functions returned by the proven factories
    // -----------------------------------------------------------------------

    /// The function a string intern-writer factory returns: it looks slot 0 up
    /// in the factory's null-prototype cache, writes `byte ^ mask()` into slot
    /// 1, and stores a new index under slot 0 on a miss.
    fn interned_string_writer(&self, out: &mut Out) {
        for factory in 0..self.fns.len() {
            for writer in self.returned_functions(factory) {
                let mut lookup = None;
                let mut stores = false;
                for kind in self.kinds(writer) {
                    match kind {
                        AstKind::BinaryExpression(binary)
                            if binary.operator == BinaryOperator::Inequality
                                && matches!(
                                    binary.right.without_parentheses(),
                                    Expression::NullLiteral(_)
                                ) =>
                        {
                            if let Expression::ComputedMemberExpression(member) =
                                binary.left.without_parentheses()
                            {
                                if self.is_slot(&member.expression, writer, 0) {
                                    if let Some(cache) = self.symbol(&member.object) {
                                        lookup = Some((cache, binary.span));
                                    }
                                }
                            }
                        }
                        AstKind::AssignmentExpression(assignment) => {
                            if let AssignmentTarget::ComputedMemberExpression(member) =
                                &assignment.left
                            {
                                stores |= self.is_slot(&member.expression, writer, 0);
                            }
                        }
                        _ => {}
                    }
                }
                let Some((cache, lookup_span)) = lookup else {
                    continue;
                };
                let dictionary = self.writes.get(&cache).is_some_and(|writes| {
                    writes.iter().any(|write| {
                        matches!(write, Write::Value(value, _) if is_null_prototype_object(value))
                    })
                });
                if !dictionary || !contains(self.fns[factory].span, self.identifier_span(cache)) {
                    continue;
                }
                let masked_writes = self
                    .kinds(writer)
                    .filter(|kind| {
                        let AstKind::AssignmentExpression(assignment) = kind else {
                            return false;
                        };
                        let AssignmentTarget::ComputedMemberExpression(member) = &assignment.left
                        else {
                            return false;
                        };
                        self.is_slot(&member.object, writer, 1)
                            && self.xors_captured_call(&assignment.right, writer)
                    })
                    .count();
                let reads_length = self.kinds(writer).any(|kind| {
                    kind_member(kind).is_some_and(|(object, name)| {
                        name == "length" && self.is_slot(object, writer, 0)
                    })
                });
                if masked_writes < 4 || !reads_length || !stores || !self.has_loop(writer) {
                    continue;
                }
                out.function(
                    "interned_string_writer",
                    self.fns[writer].span,
                    None,
                    vec![
                        format!(
                            "returned by its factory; looks slot 0 up in the factory's null-prototype cache (`{}`)",
                            self.label(lookup_span)
                        ),
                        format!(
                            "writes {masked_writes} masked bytes `slot1[..] = .. ^ mask()` and stores the new index on a miss"
                        ),
                    ],
                );
            }
        }
    }

    /// `... ^ f()` where `f` is a captured binding (the byte mask).
    fn xors_captured_call(&self, expression: &Expression<'_>, function: usize) -> bool {
        let mut found = false;
        visit_expressions(expression, &mut |inner| {
            let Expression::BinaryExpression(binary) = inner else {
                return;
            };
            if binary.operator != BinaryOperator::BitwiseXOR {
                return;
            }
            for side in [&binary.left, &binary.right] {
                if let Expression::CallExpression(call) = side.without_parentheses() {
                    found |= call.arguments.is_empty()
                        && self
                            .symbol(&call.callee)
                            .is_some_and(|symbol| self.declared_outside(symbol, function));
                }
            }
        });
        found
    }

    /// The no-argument function a byte-stream factory warms up and returns: it
    /// cycles a counter modulo 4 and returns byte lanes 0/8/16/24 of a word.
    fn keystream_next_byte(&self, out: &mut Out) {
        for factory in 0..self.fns.len() {
            for next in self.returned_functions(factory) {
                if self.slot_width(next) != 0 {
                    continue;
                }
                let modulo_four = self.binaries(next).any(|binary| {
                    binary.operator == BinaryOperator::Remainder
                        && numeric_literal(&binary.right) == Some(4.0)
                });
                let lanes: BTreeSet<u32> =
                    self.returns(next).filter_map(|argument| byte_lane(argument)).collect();
                let warmed = self.calls(factory).any(|call| {
                    call.arguments.is_empty()
                        && self.fns[next]
                            .symbol
                            .is_some_and(|symbol| self.symbol(&call.callee) == Some(symbol))
                });
                if !modulo_four || !warmed || lanes != BTreeSet::from([0, 8, 16, 24]) {
                    continue;
                }
                out.function(
                    "keystream_next_byte",
                    self.fns[next].span,
                    None,
                    vec![
                        "returned by its factory after warm-up calls; takes no arguments".to_string(),
                        "cycles a counter modulo 4 and returns byte lanes 0/8/16/24 of captured state".to_string(),
                    ],
                );
            }
        }
    }

    /// The function a no-input cipher factory returns: four slots, an output
    /// array the length of slot 0, `Math.ceil(slot0.length / n)` blocks, many
    /// XOR and unsigned-shift steps, and the array as its result.
    fn stream_cipher_apply(&self, out: &mut Out) {
        for factory in 0..self.fns.len() {
            if self.slot_width(factory) != 0 {
                continue;
            }
            for cipher in self.returned_functions(factory) {
                if self.slot_width(cipher) != 4 {
                    continue;
                }
                let output = self.own_values(cipher).into_iter().find_map(|(symbol, value)| {
                    let arguments = match value.without_parentheses() {
                        Expression::CallExpression(call)
                            if self.is_global(&call.callee, "Array") =>
                        {
                            &call.arguments
                        }
                        Expression::NewExpression(new) if self.is_global(&new.callee, "Array") => {
                            &new.arguments
                        }
                        _ => return None,
                    };
                    let [argument] = arguments.as_slice() else {
                        return None;
                    };
                    let argument = argument.as_expression()?;
                    (member_name(argument) == Some("length")
                        && member_object(argument)
                            .is_some_and(|object| self.is_slot(object, cipher, 0)))
                    .then_some(symbol)
                });
                let Some(output) = output else {
                    continue;
                };
                let blocks = self.method_calls(cipher, "ceil").any(|call| {
                    member_object(&call.callee).is_some_and(|object| self.is_global(object, "Math"))
                        && call.arguments.first().and_then(Argument::as_expression).is_some_and(
                            |argument| {
                                matches!(argument.without_parentheses(), Expression::BinaryExpression(binary)
                                    if binary.operator == BinaryOperator::Division
                                        && member_name(&binary.left) == Some("length")
                                        && member_object(&binary.left)
                                            .is_some_and(|object| self.is_slot(object, cipher, 0)))
                            },
                        )
                });
                let returns_output =
                    self.returns(cipher).any(|argument| self.symbol(argument) == Some(output));
                let xors = self.count_binary(cipher, BinaryOperator::BitwiseXOR);
                let shifts = self.count_binary(cipher, BinaryOperator::ShiftRightZeroFill);
                if !blocks || !returns_output || xors < 16 || shifts < 16 || !self.has_loop(cipher)
                {
                    continue;
                }
                out.function(
                    "stream_cipher_apply",
                    self.fns[cipher].span,
                    None,
                    vec![
                        "returned by a factory that takes no inputs; reads four argument slots".to_string(),
                        format!(
                            "allocates an output array the length of slot 0, walks it in `Math.ceil(length / n)` blocks with {xors} XOR and {shifts} unsigned-shift steps, and returns it (opaque; no algorithm identity asserted)"
                        ),
                    ],
                );
            }
        }
    }

    // -----------------------------------------------------------------------
    // Error ring
    // -----------------------------------------------------------------------

    /// `r = Object.create(null); r.message = e.message; if (e.stack != null)
    /// r.stack = e.stack; ...; ring.put(r)` over slot 0.
    fn signal_error_enqueue(&self, out: &mut Out) {
        for function in 0..self.fns.len() {
            let records: BTreeSet<SymbolId> = self
                .own_values(function)
                .into_iter()
                .filter(|(_, value)| is_null_prototype_object(value))
                .map(|(symbol, _)| symbol)
                .collect();
            let [record] = records.into_iter().collect::<Vec<_>>()[..] else {
                continue;
            };
            let mut copied = BTreeSet::new();
            for kind in self.kinds(function) {
                let AstKind::AssignmentExpression(assignment) = kind else {
                    continue;
                };
                let Some((object, name)) = target_member(&assignment.left) else {
                    continue;
                };
                if self.symbol(object) == Some(record)
                    && member_name(&assignment.right) == Some(name)
                    && member_object(&assignment.right)
                        .is_some_and(|source| self.is_slot(source, function, 0))
                {
                    copied.insert(name);
                }
            }
            if !copied.contains("message") || !copied.contains("stack") || copied.len() < 3 {
                continue;
            }
            let put = self.method_calls(function, "put").find(|call| {
                call.arguments.len() == 1
                    && call.arguments[0].as_expression().and_then(|argument| self.symbol(argument))
                        == Some(record)
                    && member_object(&call.callee)
                        .and_then(|object| self.symbol(object))
                        .is_some_and(|ring| self.declared_outside(ring, function))
            });
            let Some(put) = put else {
                continue;
            };
            out.function(
                "signal_error_enqueue",
                self.fns[function].span,
                None,
                vec![
                    format!(
                        "copies {} error fields of slot 0 into a null-prototype record",
                        copied.len()
                    ),
                    format!("puts the record into a captured ring: `{}`", self.label(put.span)),
                ],
            );
        }
    }

    /// A ring buffer in either shape: class methods or prototype assignments.
    fn ring_buffers(&self, out: &mut Out) {
        for node in self.ctx.nodes.iter() {
            let AstKind::Class(class) = node.kind() else {
                continue;
            };
            let mut constructor = None;
            let mut methods = BTreeMap::new();
            for element in &class.body.body {
                let ClassElement::MethodDefinition(method) = element else {
                    continue;
                };
                let (Some(name), Some(index)) =
                    (method.key.static_name(), self.method_function(method))
                else {
                    continue;
                };
                if method.kind == MethodDefinitionKind::Constructor {
                    constructor = Some(index);
                } else if method.kind == MethodDefinitionKind::Method {
                    methods.insert(name.to_string(), index);
                }
            }
            if let Some(constructor) = constructor {
                if self.ring_buffer(out, constructor, &methods)
                    && class.id.as_ref().and_then(|id| id.symbol_id.get()).is_some()
                {
                    out.class(
                        "ring_buffer_class",
                        class.span,
                        vec![
                            "class declaration whose constructor initializes two zero fields, an empty backing array and a slot-0 capacity, and whose put/toArray/clear methods use them".to_string(),
                        ],
                    );
                }
            }
        }
        for (symbol, methods) in &self.prototype_methods {
            if let Some(constructor) = self.by_symbol.get(symbol) {
                self.ring_buffer(out, *constructor, methods);
            }
        }
    }

    /// The constructor sets two zero fields, an empty backing array and a
    /// capacity from slot 0 (Infinity otherwise) on `this`; `put`, `toArray`
    /// and `clear` use exactly those fields.
    fn ring_buffer(&self, out: &mut Out, constructor: usize, methods: &BTreeMap<String, usize>) -> bool {
        let (Some(&put), Some(&to_array), Some(&clear)) =
            (methods.get("put"), methods.get("toArray"), methods.get("clear"))
        else {
            return false;
        };
        let mut fields: BTreeMap<&str, &Expression> = BTreeMap::new();
        for kind in self.kinds(constructor) {
            if let AstKind::AssignmentExpression(assignment) = kind {
                if let Some((object, name)) = target_member(&assignment.left) {
                    if matches!(object.without_parentheses(), Expression::ThisExpression(_)) {
                        fields.insert(name, &assignment.right);
                    }
                }
            }
        }
        let select = |test: &dyn Fn(&Expression) -> bool| -> Vec<&str> {
            fields.iter().filter(|(_, value)| test(value)).map(|(name, _)| *name).collect()
        };
        let arrays = select(&is_empty_array);
        let zeros = select(&|value| numeric_literal(value) == Some(0.0));
        let capacities = select(&|value| {
            self.mentions_global(value, "Infinity") && self.mentions_slot(value, constructor, 0)
        });
        let ([array], [capacity], 2) = (arrays.as_slice(), capacities.as_slice(), zeros.len())
        else {
            return false;
        };
        let (array, capacity) = (*array, *capacity);
        let cursor = self.kinds(put).find_map(|kind| {
            let AstKind::AssignmentExpression(assignment) = kind else {
                return None;
            };
            let AssignmentTarget::ComputedMemberExpression(member) = &assignment.left else {
                return None;
            };
            (this_field(&member.object) == Some(array) && self.is_slot(&assignment.right, put, 0))
                .then(|| this_field(&member.expression))?
        });
        let Some(cursor) = cursor.filter(|cursor| zeros.contains(cursor)) else {
            return false;
        };
        let wraps = self.binaries(put).any(|binary| {
            binary.operator == BinaryOperator::GreaterEqualThan
                && this_field(&binary.left) == Some(cursor)
                && this_field(&binary.right) == Some(capacity)
        });
        let snapshot = self.method_calls(to_array, "slice").count() >= 2
            && self.method_calls(to_array, "concat").count() == 1;
        let clears = self.kinds(clear).any(|kind| {
            matches!(kind, AstKind::AssignmentExpression(assignment)
                if is_empty_array(&assignment.right)
                    && target_member(&assignment.left).is_some_and(|(object, name)| {
                        matches!(object.without_parentheses(), Expression::ThisExpression(_))
                            && name == array
                    }))
        });
        if !wraps || !snapshot || !clears {
            return false;
        }
        let shared = format!(
            "ring buffer: backing array `{array}`, cursor `{cursor}`, capacity `{capacity}`"
        );
        out.function(
            "ring_buffer_constructor",
            self.fns[constructor].span,
            None,
            vec![
                shared.clone(),
                "initializes cursor and length to 0, the capacity from slot 0 and an empty backing array".to_string(),
            ],
        );
        out.function(
            "ring_buffer_put",
            self.fns[put].span,
            None,
            vec![
                shared.clone(),
                "writes slot 0 at the cursor, grows the length and wraps the cursor at the capacity".to_string(),
            ],
        );
        out.function(
            "ring_buffer_to_array",
            self.fns[to_array].span,
            None,
            vec![shared.clone(), "returns an oldest-first copy through slice/concat".to_string()],
        );
        out.function(
            "ring_buffer_clear",
            self.fns[clear].span,
            None,
            vec![shared, "resets the backing array".to_string()],
        );
        true
    }

    // -----------------------------------------------------------------------
    // Emoji glyph pixel probe
    // -----------------------------------------------------------------------

    /// A collector that draws every `{emoji, x, y, component}` entry of a MAIN
    /// table on a canvas and reads one pixel component per entry.
    fn emoji_probe(&self, out: &mut Out) {
        for function in 0..self.fns.len() {
            let names = self.member_names(function);
            if !["getContext", "fillText", "getImageData", "emoji", "component"]
                .iter()
                .all(|name| names.contains(name))
            {
                continue;
            }
            let canvas = self.method_calls(function, "createElement").any(|call| {
                matches!(call.arguments.first().and_then(Argument::as_expression),
                    Some(Expression::StringLiteral(literal)) if literal.value == "canvas")
            });
            if !canvas || !self.has_loop(function) {
                continue;
            }
            let tables: BTreeSet<SymbolId> = self
                .captured_reads(function)
                .into_iter()
                .filter(|symbol| self.single_value(*symbol).is_some_and(is_emoji_probe_table))
                .collect();
            let [table] = tables.into_iter().collect::<Vec<_>>()[..] else {
                continue;
            };
            out.function(
                "emoji_glyph_pixel_collector",
                self.fns[function].span,
                None,
                vec![
                    "creates a canvas, draws each table entry's emoji with fillText and reads one pixel component through getImageData".to_string(),
                    "iterates a captured table of {emoji, x, y, component} records".to_string(),
                ],
            );
            out.main(
                "emoji_glyph_pixel_probes",
                self.identifier_span(table),
                vec!["array of {emoji, x, y, component} records drawn and sampled by the emoji glyph collector".to_string()],
            );
        }
    }

    // -----------------------------------------------------------------------
    // Bundle framing
    // -----------------------------------------------------------------------

    /// The bundle encoder: one slot whose array fields are framed with 7-bit
    /// varint lengths and turned into 6-bit text through `alphabet.charAt`
    /// lookups over 3-byte groups. Its factory returns it.
    fn bundle_encoder(&self, out: &mut Out) {
        for factory in 0..self.fns.len() {
            for encoder in self.returned_functions(factory) {
                if self.slot_width(encoder) != 1 {
                    continue;
                }
                let char_at = self.method_calls(encoder, "charAt").count();
                let varint_shifts = self
                    .binaries(encoder)
                    .filter(|binary| {
                        binary.operator == BinaryOperator::ShiftRightZeroFill
                            && numeric_literal(&binary.right) == Some(7.0)
                    })
                    .count();
                let mod_three = self.binaries(encoder).any(|binary| {
                    binary.operator == BinaryOperator::Remainder
                        && numeric_literal(&binary.right) == Some(3.0)
                });
                let fields: BTreeSet<&str> = self
                    .kinds(encoder)
                    .filter_map(|kind| {
                        let (object, name) = kind_member(kind)?;
                        if name != "length" {
                            return None;
                        }
                        let field = member_name(object)?;
                        (self.slot(member_object(object)?, encoder) == Some(0)).then_some(field)
                    })
                    .collect();
                if char_at < 4 || varint_shifts < 4 || !mod_three || fields.len() < 3 {
                    continue;
                }
                let framing = format!(
                    "reads {} array fields of slot 0 and frames them with {varint_shifts} 7-bit varint shifts",
                    fields.len()
                );
                out.function(
                    "signal_bundle_encoder",
                    self.fns[encoder].span,
                    None,
                    vec![
                        format!("returned by its factory; {framing}"),
                        format!("emits 6-bit text through {char_at} `alphabet.charAt` lookups over 3-byte groups"),
                    ],
                );
                out.function(
                    "signal_bundle_encoder_factory",
                    self.fns[factory].span,
                    Some("signal_bundle_encoder_factory_alias"),
                    vec![format!("returns the signal bundle encoder, which {framing}")],
                );
                self.bundle_value_encoder(out, factory, encoder);
                self.bundle_entropy_counter(out, factory, encoder);
                self.bundle_extensions(out, factory);
            }
        }
    }

    /// A captured `x(...).encode` binding called on a slot-0 field.
    fn bundle_value_encoder(&self, out: &mut Out, factory: usize, encoder: usize) {
        for call in self.calls(encoder) {
            let Some(callee) = self.symbol(&call.callee) else {
                continue;
            };
            let on_field = call
                .arguments
                .first()
                .and_then(Argument::as_expression)
                .and_then(member_object)
                .is_some_and(|object| self.slot(object, encoder) == Some(0));
            let encode_member = self.single_value(callee).is_some_and(|value| {
                member_name(value) == Some("encode")
                    && matches!(
                        member_object(value).map(Expression::without_parentheses),
                        Some(Expression::CallExpression(_))
                    )
            });
            if on_field && encode_member && self.declared_outside(callee, factory) {
                out.main(
                    "bundle_value_encoder",
                    self.identifier_span(callee),
                    vec![format!(
                        "the `.encode` of a configured codec, called by the bundle encoder on slot-0 fields: `{}`",
                        self.label(call.span)
                    )],
                );
            }
        }
    }

    /// MAIN `let c = 0`, otherwise written only by `++c`, incremented by the
    /// encoder and XORed into the words it prefixes with `unshift`.
    fn bundle_entropy_counter(&self, out: &mut Out, factory: usize, encoder: usize) {
        let Some(unshift) = self.method_calls(encoder, "unshift").next() else {
            return;
        };
        for symbol in self.captured_reads(encoder) {
            let Some(writes) = self.writes.get(&symbol) else {
                continue;
            };
            let zero_inits = writes
                .iter()
                .filter(|write| {
                    matches!(write, Write::Value(value, _) if numeric_literal(value) == Some(0.0))
                })
                .count();
            let increments = writes.iter().filter(|write| matches!(write, Write::Increment)).count();
            let encoder_increments = self
                .kinds(encoder)
                .filter(|kind| {
                    matches!(kind, AstKind::UpdateExpression(update)
                        if matches!(&update.argument, SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier)
                            if crate::oxc::identifier_symbol(identifier, self.ctx.scoping) == Some(symbol)))
                })
                .count();
            let xored = self.binaries(encoder).any(|binary| {
                binary.operator == BinaryOperator::BitwiseXOR
                    && (self.symbol(&binary.right) == Some(symbol)
                        || self.symbol(&binary.left) == Some(symbol))
            });
            if zero_inits == 1
                && zero_inits + increments == writes.len()
                && encoder_increments > 0
                && xored
                && self.declared_outside(symbol, factory)
            {
                out.main(
                    "bundle_entropy_counter",
                    self.identifier_span(symbol),
                    vec![
                        "counter initialized 0 and otherwise written only by `++`".to_string(),
                        format!(
                            "the bundle encoder increments it {encoder_increments} time(s) and XORs it into the words it prefixes with `{}`",
                            self.label(unshift.span)
                        ),
                    ],
                );
            }
        }
    }

    /// Aliases of `{detector, serialiser}` objects the factory reads into its
    /// extensions map, told apart by what the detector and serialiser do.
    fn bundle_extensions(&self, out: &mut Out, factory: usize) {
        for symbol in self.captured_reads(factory) {
            let Some(target) = self.single_value(symbol).and_then(|value| self.symbol(value))
            else {
                continue;
            };
            let Some(Expression::ObjectExpression(object)) =
                self.single_value(target).map(Expression::without_parentheses)
            else {
                continue;
            };
            let members = object_members_of(object);
            let member = |name: &str| {
                members.iter().find(|member| member.name == name).map(|member| member.value)
            };
            let (Some(detector), Some(serialiser)) = (
                member("detector").and_then(|value| self.function_value(value)),
                member("serialiser").and_then(|value| self.function_value(value)),
            ) else {
                continue;
            };
            let returns: Vec<&Expression> = self.returns(detector).collect();
            let [returned] = returns.as_slice() else {
                continue;
            };
            let throws = self.kinds(serialiser).any(|kind| matches!(kind, AstKind::ThrowStatement(_)));
            match returned.without_parentheses() {
                Expression::BooleanLiteral(literal) if !literal.value && throws => {
                    out.main(
                        "bundle_decode_only_extension",
                        self.identifier_span(symbol),
                        vec!["alias of a codec extension the bundle encoder factory configures: its detector never matches and its serialiser throws, so it only decodes".to_string()],
                    );
                }
                Expression::BinaryExpression(binary)
                    if binary.operator == BinaryOperator::StrictEquality
                        && self.slot(&binary.left, detector) == Some(0)
                        && self
                            .symbol(&binary.right)
                            .and_then(|sentinel| self.single_value(sentinel))
                            .is_some_and(is_null_prototype_object) =>
                {
                    out.main(
                        "bundle_sentinel_extension",
                        self.identifier_span(symbol),
                        vec!["alias of a codec extension the bundle encoder factory configures: its detector matches the identity of a null-prototype sentinel".to_string()],
                    );
                }
                _ => {}
            }
        }
    }

    // -----------------------------------------------------------------------
    // Batch scheduler alias
    // -----------------------------------------------------------------------

    /// An alias of a class whose `schedule` method calls `setTimeout`, used as
    /// `new alias(budget).schedule(task)`.
    fn batch_scheduler_alias(&self, out: &mut Out) {
        let defers = |index: usize| {
            self.calls(index).any(|call| self.is_global(&call.callee, "setTimeout"))
        };
        let mut schedulers = BTreeSet::new();
        for node in self.ctx.nodes.iter() {
            let AstKind::Class(class) = node.kind() else {
                continue;
            };
            let Some(symbol) = class.id.as_ref().and_then(|id| id.symbol_id.get()) else {
                continue;
            };
            let schedules = class.body.body.iter().any(|element| {
                let ClassElement::MethodDefinition(method) = element else {
                    return false;
                };
                method.key.static_name().as_deref() == Some("schedule")
                    && self.method_function(method).is_some_and(defers)
            });
            if schedules {
                schedulers.insert(symbol);
            }
        }
        for (symbol, methods) in &self.prototype_methods {
            if methods.get("schedule").is_some_and(|index| defers(*index)) {
                schedulers.insert(*symbol);
            }
        }
        for (&symbol, writes) in &self.writes {
            let [Write::Value(value, _)] = writes.as_slice() else {
                continue;
            };
            if !self.symbol(value).is_some_and(|target| schedulers.contains(&target)) {
                continue;
            }
            let site = self.ctx.nodes.iter().find_map(|node| {
                let AstKind::CallExpression(call) = node.kind() else {
                    return None;
                };
                if member_name(&call.callee) != Some("schedule") {
                    return None;
                }
                let Some(Expression::NewExpression(new)) =
                    member_object(&call.callee).map(Expression::without_parentheses)
                else {
                    return None;
                };
                (self.symbol(&new.callee) == Some(symbol)).then_some(call.span)
            });
            if let Some(site) = site {
                out.main(
                    "batch_scheduler_alias",
                    self.identifier_span(symbol),
                    vec![
                        "alias of a class whose schedule method defers work with setTimeout".to_string(),
                        format!("constructed with a budget and scheduled: `{}`", self.label(site)),
                    ],
                );
            }
        }
    }
}

// ===========================================================================
// Expression readers
// ===========================================================================

fn contains(outer: Span, inner: Span) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

fn numeric_literal(expression: &Expression<'_>) -> Option<f64> {
    match expression.without_parentheses() {
        Expression::NumericLiteral(literal) => Some(literal.value),
        _ => None,
    }
}

fn is_empty_array(expression: &Expression<'_>) -> bool {
    matches!(expression.without_parentheses(), Expression::ArrayExpression(array)
        if array.elements.is_empty())
}

fn member_name<'e>(expression: &'e Expression<'_>) -> Option<&'e str> {
    match expression.without_parentheses() {
        Expression::StaticMemberExpression(member) => Some(member.property.name.as_str()),
        Expression::ComputedMemberExpression(member) => match &member.expression {
            Expression::StringLiteral(literal) => Some(literal.value.as_str()),
            _ => None,
        },
        _ => None,
    }
}

fn member_object<'e, 'a>(expression: &'e Expression<'a>) -> Option<&'e Expression<'a>> {
    match expression.without_parentheses() {
        Expression::StaticMemberExpression(member) => Some(&member.object),
        Expression::ComputedMemberExpression(member) => Some(&member.object),
        _ => None,
    }
}

/// `(object, name)` of a member node with a static name.
fn kind_member<'a>(kind: AstKind<'a>) -> Option<(&'a Expression<'a>, &'a str)> {
    match kind {
        AstKind::StaticMemberExpression(member) => {
            Some((&member.object, member.property.name.as_str()))
        }
        AstKind::ComputedMemberExpression(member) => match &member.expression {
            Expression::StringLiteral(literal) => Some((&member.object, literal.value.as_str())),
            _ => None,
        },
        _ => None,
    }
}

/// `(object, name)` of a member assignment target with a static name.
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

/// `this.name` / `this["name"]`.
fn this_field<'e>(expression: &'e Expression<'_>) -> Option<&'e str> {
    let object = member_object(expression)?;
    if matches!(object.without_parentheses(), Expression::ThisExpression(_)) {
        member_name(expression)
    } else {
        None
    }
}

/// `Object.create(null)`.
fn is_null_prototype_object(expression: &Expression<'_>) -> bool {
    let Expression::CallExpression(call) = expression.without_parentheses() else {
        return false;
    };
    member_name(&call.callee) == Some("create")
        && matches!(member_object(&call.callee).map(Expression::without_parentheses),
            Some(Expression::Identifier(identifier)) if identifier.name == "Object")
        && matches!(call.arguments.as_slice(), [argument]
            if matches!(argument.as_expression(), Some(Expression::NullLiteral(_))))
}

/// Leaves of a `&&` chain.
fn and_leaves<'e, 'a>(expression: &'e Expression<'a>, out: &mut Vec<&'e Expression<'a>>) {
    match expression.without_parentheses() {
        Expression::LogicalExpression(logical) if logical.operator == LogicalOperator::And => {
            and_leaves(&logical.left, out);
            and_leaves(&logical.right, out);
        }
        other => out.push(other),
    }
}

/// The byte lane `x & 255`, `x >> k & 255` or `x >>> k & 255` selects.
fn byte_lane(expression: &Expression<'_>) -> Option<u32> {
    let Expression::BinaryExpression(binary) = expression.without_parentheses() else {
        return None;
    };
    if binary.operator != BinaryOperator::BitwiseAnd
        || numeric_literal(&binary.right) != Some(255.0)
    {
        return None;
    }
    match binary.left.without_parentheses() {
        Expression::Identifier(_) => Some(0),
        Expression::BinaryExpression(shift)
            if matches!(
                shift.operator,
                BinaryOperator::ShiftRight | BinaryOperator::ShiftRightZeroFill
            ) && matches!(shift.left.without_parentheses(), Expression::Identifier(_)) =>
        {
            numeric_literal(&shift.right).map(|value| value as u32)
        }
        _ => None,
    }
}

/// Every sub-expression of a (non-function) expression tree.
fn visit_expressions<'e, 'a>(
    expression: &'e Expression<'a>,
    visit: &mut dyn FnMut(&'e Expression<'a>),
) {
    visit(expression);
    match expression {
        Expression::ParenthesizedExpression(inner) => visit_expressions(&inner.expression, visit),
        Expression::BinaryExpression(binary) => {
            visit_expressions(&binary.left, visit);
            visit_expressions(&binary.right, visit);
        }
        Expression::LogicalExpression(logical) => {
            visit_expressions(&logical.left, visit);
            visit_expressions(&logical.right, visit);
        }
        Expression::ConditionalExpression(conditional) => {
            visit_expressions(&conditional.test, visit);
            visit_expressions(&conditional.consequent, visit);
            visit_expressions(&conditional.alternate, visit);
        }
        Expression::UnaryExpression(unary) => visit_expressions(&unary.argument, visit),
        Expression::StaticMemberExpression(member) => visit_expressions(&member.object, visit),
        Expression::ComputedMemberExpression(member) => {
            visit_expressions(&member.object, visit);
            visit_expressions(&member.expression, visit);
        }
        Expression::CallExpression(call) => {
            visit_expressions(&call.callee, visit);
            for argument in &call.arguments {
                if let Some(argument) = argument.as_expression() {
                    visit_expressions(argument, visit);
                }
            }
        }
        Expression::SequenceExpression(sequence) => {
            for inner in &sequence.expressions {
                visit_expressions(inner, visit);
            }
        }
        _ => {}
    }
}

/// `[{emoji, x, y, component}, ...]` with at least one entry.
fn is_emoji_probe_table<'a>(expression: &'a Expression<'a>) -> bool {
    let Some(elements) = array_elements(expression.without_parentheses()) else {
        return false;
    };
    !elements.is_empty()
        && elements.iter().all(|element| {
            object_members(element).is_some_and(|members| {
                let names: BTreeSet<&str> =
                    members.iter().map(|member| member.name.as_str()).collect();
                names == BTreeSet::from(["emoji", "x", "y", "component"])
                    && members.iter().any(|member| {
                        member.name == "emoji"
                            && matches!(member.value, Expression::StringLiteral(_))
                    })
            })
        })
}
