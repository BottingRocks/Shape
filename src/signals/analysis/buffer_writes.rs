//! The writes into a slot's emitted buffer, matched to the view's emission
//! leaves, with slot-local facts for each written value.
//!
//! A wave producer serves five slots, so its member names are shared by the
//! row; these facts come from the slot's own sites instead:
//!
//! * the emission sites among the slot anchors (`obs.next([[], buf])`), the
//!   buffer they send and every write into that buffer (`buf[i] = v ^ ks()`
//!   or `writer(…, v, buf, pos)`), in source order;
//! * a backward slice of each written value: the expression itself and, for
//!   every local binding it reads, the textually latest definition before the
//!   read (register reuse makes all-definitions unions useless), bounded;
//! * for a value read from a boolean flag array (`flags[1] | flags[0] << 1`),
//!   the context of every element store `flags[k] = …`: the enclosing `try`
//!   (crossing a getter function), or the enclosing `if`.
//!
//! Facts are member names, string literals, regex sources, unbound globals,
//! called names and console reads on a callback parameter. Generated
//! spellings never enter a fact set. The per-write facts are memoized per
//! analysis in [`FinderCtx::memo`], keyed by the slot view's own structure
//! (producer span, anchors, shapes); binders read what their recognizer
//! built.
//!
//! Used by: `BrowserApiOwnPropertyFlags`, `BrowserApiProxyFlags`,
//! `BuiltinProbeChecksum`, `FingerprintApiIntegrity`,
//! `FunctionToStringComparison`, `GrecaptchaAccessorHookFlags`,
//! `NativeApiIntegrityFlags`, `NonNativeBuiltinCount`,
//! `NullDerefTypeErrorFields`, `ProxyTamperFlags`, `VmSourceIntegrity`; helper
//! modules [`flag_bytes`](super::flag_bytes),
//! [`native_checks`](super::native_checks).

use std::collections::{BTreeSet, HashSet};

use oxc_ast::ast::*;
use oxc_ast::AstKind;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{NodeId, SymbolId};
use oxc_span::GetSpan;
use oxc_syntax::operator::BinaryOperator;
use oxc_syntax::scope::ScopeFlags;

use crate::signals::extract::wiring::LifecycleRole;
use super::slot_view::generated;
use super::{FinderCtx, LeafOp, SlotView};

/// Facts of a slice or a context.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Facts {
    pub members: BTreeSet<String>,
    pub strings: BTreeSet<String>,
    pub regex: BTreeSet<String>,
    pub globals: BTreeSet<String>,
    pub calls: BTreeSet<String>,
    /// `p.console` read where `p` is a function parameter.
    pub param_console: bool,
    /// Regex sources and called names inside function literals of the
    /// slice (inline callbacks such as `list.filter(f => /re/.test(f))`);
    /// kept apart because a callback's body is not the value itself.
    pub callback_regex: BTreeSet<String>,
    pub callback_calls: BTreeSet<String>,
}

impl Facts {
    /// Add every fact of `other`.
    pub fn extend(&mut self, other: &Facts) {
        self.members.extend(other.members.iter().cloned());
        self.strings.extend(other.strings.iter().cloned());
        self.regex.extend(other.regex.iter().cloned());
        self.globals.extend(other.globals.iter().cloned());
        self.calls.extend(other.calls.iter().cloned());
        self.param_console |= other.param_console;
        self.callback_regex.extend(other.callback_regex.iter().cloned());
        self.callback_calls.extend(other.callback_calls.iter().cloned());
    }

    /// A member `name` was read.
    pub fn member(&self, name: &str) -> bool {
        self.members.contains(name)
    }

    /// The string literal `value` occurs.
    pub fn string(&self, value: &str) -> bool {
        self.strings.contains(value)
    }

    /// A call of `name` occurs.
    pub fn call(&self, name: &str) -> bool {
        self.calls.contains(name)
    }

    /// The unbound global `name` is read.
    pub fn global(&self, name: &str) -> bool {
        self.globals.contains(name)
    }

    /// How many of `values` occur as string literals.
    pub fn count_strings(&self, values: &[&str]) -> usize {
        values.iter().filter(|value| self.string(value)).count()
    }

    /// How many of `names` are read as members.
    pub fn count_members(&self, names: &[&str]) -> usize {
        names.iter().filter(|name| self.member(name)).count()
    }
}

/// One write into an emitted buffer.
#[derive(Clone, Debug, Default)]
pub(crate) struct WriteInfo {
    /// `buf[i] = v ^ ks()` (a byte) rather than a writer call.
    pub byte: bool,
    /// The written value is a numeric literal (a null marker).
    pub literal: bool,
    /// Slice of the written value.
    pub facts: Facts,
    /// The value is `x < 1 ? x : …` (a fraction normalisation).
    pub fraction: bool,
    /// Slice of `x` in a padded `… (x & 255 ^ pad)` value.
    pub padded: Option<Facts>,
    /// `x` of a padded value bound only to a numeric literal: the
    /// definition's offset and the value.
    pub folded: Option<(u32, f64)>,
}

/// One emission with its writes, matched to a view emission.
#[derive(Clone, Debug, Default)]
pub(crate) struct EmissionInfo {
    pub writes: Vec<WriteInfo>,
}

/// Slot-local structure.
#[derive(Clone, Debug, Default)]
pub(crate) struct SlotInfo {
    /// Per view emission: the AST emissions with the same phase and op
    /// sequence (empty: no match).
    pub emissions: Vec<Vec<EmissionInfo>>,
    /// Union of every write slice.
    pub facts: Facts,
    /// Contexts of flag-array element stores.
    pub stores: Vec<Facts>,
}

impl SlotInfo {
    /// The facts of every flag-store context, merged.
    pub fn store_facts(&self) -> Facts {
        let mut out = Facts::default();
        self.stores.iter().for_each(|store| out.extend(store));
        out
    }
}

// ---------------------------------------------------------------------------
// Caches
// ---------------------------------------------------------------------------

/// Memo keys of this module (per analysis, in [`FinderCtx::memo`]).
#[derive(Hash, PartialEq, Eq)]
struct StartsKey;

/// Node ids sorted by start offset.
struct Starts(Vec<(u32, u32)>);

#[derive(Hash, PartialEq, Eq)]
struct SlotKey(u32, u32, Vec<u32>, String);

fn slot_key(view: &SlotView) -> SlotKey {
    let producer = view.producer_span();
    SlotKey(
        producer.map_or(0, |span| span.start),
        producer.map_or(0, |span| span.end),
        view.anchors().to_vec(),
        view.shapes().join("|"),
    )
}

/// The slot-local structure of `view`, computed once per slot and
/// analysis (binders read the structure their recognizer built).
pub(crate) fn slot_info(view: &SlotView, ctx: &FinderCtx) -> SlotInfo {
    let info = ctx.memo(slot_key(view), || {
        let starts = ctx.memo(StartsKey, || {
            let mut starts: Vec<(u32, u32)> = ctx
                .parse
                .nodes
                .iter()
                .map(|node| (node.kind().span().start, node.id().index() as u32))
                .collect();
            starts.sort_unstable();
            Starts(starts)
        });
        let walker = Walker {
            ctx,
            starts: &starts.0,
        };
        walker.slot(view)
    });
    (*info).clone()
}

// ---------------------------------------------------------------------------
// Walker
// ---------------------------------------------------------------------------

const SLICE_DEPTH: usize = 7;
/// Register tuples followed per element read (`p = [q[0], …]; p[0]`).
const TUPLE_DEPTH: usize = 8;

/// An emitted buffer: a binding, or an empty array literal held in a
/// register tuple (`b = [[], []]; b[1][i] = …; obs.next([b[0], b[1]])`),
/// named by the literal's span.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BufferRef {
    Binding(SymbolId),
    Literal(u32, u32),
}
const SLICE_BUDGET: usize = 48;

struct Walker<'w, 'c, 's, 'a> {
    ctx: &'w FinderCtx<'c, 's, 'a>,
    starts: &'w [(u32, u32)],
}

struct Def<'a> {
    node: NodeId,
    start: u32,
    end: u32,
    function: Option<NodeId>,
    rhs: Option<&'a Expression<'a>>,
}

impl<'w, 'c, 's, 'a> Walker<'w, 'c, 's, 'a> {
    fn nodes(&self) -> &'a oxc_semantic::AstNodes<'a> {
        self.ctx.parse.nodes
    }

    fn scoping(&self) -> &'a oxc_semantic::Scoping {
        self.ctx.parse.scoping
    }

    fn nodes_at(&self, offset: u32) -> Vec<NodeId> {
        let first = self.starts.partition_point(|(start, _)| *start < offset);
        self.starts[first..]
            .iter()
            .take_while(|(start, _)| *start == offset)
            .map(|(_, id)| NodeId::new(*id as usize))
            .collect()
    }

    /// The patterns of a binding whose every write is `RegExp("…")` or
    /// `new RegExp("…")` with a literal pattern (any realm's `RegExp`), or a
    /// copy of such a binding; empty when some write is anything else.
    fn regexp_patterns(&self, symbol: SymbolId) -> Vec<String> {
        self.regexp_patterns_within(symbol, 2)
    }

    fn regexp_patterns_within(&self, symbol: SymbolId, copies: usize) -> Vec<String> {
        let pattern = |expression: &Expression<'a>| -> Option<Vec<String>> {
            let (callee, arguments) = match expression.without_parentheses() {
                Expression::CallExpression(call) => (&call.callee, &call.arguments),
                Expression::NewExpression(new) => (&new.callee, &new.arguments),
                Expression::Identifier(identifier) if copies > 0 => {
                    let source = self.symbol(identifier).filter(|source| *source != symbol)?;
                    let found = self.regexp_patterns_within(source, copies - 1);
                    return (!found.is_empty()).then_some(found);
                }
                _ => return None,
            };
            let named = match callee.without_parentheses() {
                Expression::Identifier(identifier) => identifier.name == "RegExp" && self.symbol(identifier).is_none(),
                Expression::StaticMemberExpression(member) => member.property.name == "RegExp",
                _ => false,
            };
            match arguments.first().and_then(|argument| argument.as_expression()) {
                Some(Expression::StringLiteral(literal)) if named => Some(vec![literal.value.to_string()]),
                _ => None,
            }
        };
        let mut out = Vec::new();
        let declaration = self.scoping().symbol_declaration(symbol);
        if let AstKind::VariableDeclarator(declarator) = self.nodes().kind(declaration) {
            if let Some(init) = &declarator.init {
                let Some(found) = pattern(init) else { return Vec::new() };
                out.extend(found);
            }
        }
        for reference in self.scoping().get_resolved_references(symbol) {
            if !reference.is_write() {
                continue;
            }
            let AstKind::AssignmentExpression(assignment) = self.nodes().parent_kind(reference.node_id()) else {
                return Vec::new();
            };
            let Some(found) = assignment.operator.is_assign().then(|| pattern(&assignment.right)).flatten() else {
                return Vec::new();
            };
            out.extend(found);
        }
        out
    }

    fn enclosing_function(&self, id: NodeId) -> Option<NodeId> {
        self.nodes().ancestor_ids(id).find(|ancestor| {
            matches!(
                self.nodes().kind(*ancestor),
                AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)
            )
        })
    }

    fn symbol(&self, identifier: &IdentifierReference<'a>) -> Option<SymbolId> {
        crate::analysis::identifier_symbol(identifier, self.scoping())
    }

    fn is_parameter(&self, symbol: SymbolId) -> bool {
        let declaration = self.scoping().symbol_declaration(symbol);
        matches!(self.nodes().kind(declaration), AstKind::FormalParameter(_))
            || matches!(self.nodes().parent_kind(declaration), AstKind::FormalParameter(_))
    }

    /// The buffer an emission site sends: `obs.next([[], buf])`, or
    /// `next.call(obs, pair)` with `pair` written as `[[], buf]` or bound
    /// to it just before.
    /// A pair bound in both arms of an `if` (`gate ? [[], a] : [[], b]`
    /// written as statements) sends either arm's buffer: all are returned.
    fn emission_buffers(&self, call: &'a CallExpression<'a>) -> Vec<&'a IdentifierReference<'a>> {
        if let Some(buffer) = emitted_buffer(call) {
            return vec![buffer];
        }
        let Expression::StaticMemberExpression(callee) = call.callee.without_parentheses() else {
            return Vec::new();
        };
        // `next.call(obs, pair)` or `obs.next(pair)` with `pair` a binding.
        let pair = match (callee.property.name.as_str(), call.arguments.len()) {
            ("call", 2) => call.arguments[1].as_expression(),
            ("next", 1) => call.arguments[0].as_expression(),
            _ => None,
        };
        let Some(pair) = pair else {
            return Vec::new();
        };
        if let Some(buffer) = pair_buffer(pair) {
            return vec![buffer];
        }
        let Expression::Identifier(pair) = pair.without_parentheses() else {
            return Vec::new();
        };
        let Some(symbol) = self.symbol(pair) else {
            return Vec::new();
        };
        let defs: Vec<Def<'a>> = self.defs(symbol).into_iter().filter(|def| def.end <= call.span.start).collect();
        let Some(last) = defs.iter().max_by_key(|def| def.end) else {
            return Vec::new();
        };
        // `pair = []; pair[0] = list; pair[1] = buf; next.call(obs, pair)`
        // or `pair = [list]; pair[1] = buf; obs.next(pair)`: the pair
        // filled element-wise after a short array.
        if matches!(last.rhs.map(Expression::without_parentheses),
            Some(Expression::ArrayExpression(array)) if array.elements.len() <= 1
                && array.elements.iter().all(|element| element.as_expression().is_some()))
        {
            let stored: Vec<&'a IdentifierReference<'a>> = self
                .scoping()
                .get_resolved_references(symbol)
                .filter_map(|reference| {
                    let id = reference.node_id();
                    let start = self.nodes().kind(id).span().start;
                    if start < last.end || start >= call.span.start {
                        return None;
                    }
                    let AstKind::ComputedMemberExpression(member) = self.nodes().parent_kind(id) else {
                        return None;
                    };
                    if !matches!(member.expression.without_parentheses(),
                        Expression::NumericLiteral(index) if index.value == 1.0)
                    {
                        return None;
                    }
                    let member_id = self.nodes().parent_id(id);
                    self.nodes().ancestor_ids(member_id).take(2).find_map(|ancestor| match self.nodes().kind(ancestor) {
                        AstKind::AssignmentExpression(assignment) if assignment.left.span() == member.span => {
                            match assignment.right.without_parentheses() {
                                Expression::Identifier(buffer) => Some(&**buffer),
                                _ => None,
                            }
                        }
                        _ => None,
                    })
                })
                .collect();
            return match stored.as_slice() {
                [buffer] => vec![*buffer],
                _ => Vec::new(),
            };
        }
        let arms: Vec<&Def<'a>> = defs
            .iter()
            .filter(|def| def.end == last.end || self.exclusive(def.node, last.start))
            .collect();
        let buffers: Vec<&'a IdentifierReference<'a>> =
            arms.iter().filter_map(|def| def.rhs.and_then(pair_buffer)).collect();
        if buffers.len() == arms.len() {
            buffers
        } else {
            Vec::new()
        }
    }

    /// [`Self::emission_buffers`], else the buffer of a pair whose buffer
    /// element is read out of a register tuple (`obs.next([[], t[0]])`,
    /// `obs.next([t[0], t[1]])`) or stored into a short pair through an
    /// inline store helper (`p = []; p[1] = ((a, b, c, d) => { a[b] = c;
    /// return d; })(p, 0, [], buf); next.call(obs, p)`).
    fn emission_refs(&self, call: &'a CallExpression<'a>) -> Vec<BufferRef> {
        let buffers = self.emission_buffers(call);
        if !buffers.is_empty() {
            return buffers
                .iter()
                .map(|buffer| self.symbol(buffer).map(BufferRef::Binding))
                .collect::<Option<Vec<_>>>()
                .unwrap_or_default();
        }
        let Expression::StaticMemberExpression(callee) = call.callee.without_parentheses() else {
            return Vec::new();
        };
        let pair = match (callee.property.name.as_str(), call.arguments.len()) {
            ("call", 2) => call.arguments[1].as_expression(),
            ("next", 1) => call.arguments[0].as_expression(),
            _ => None,
        };
        let Some(pair) = pair else {
            return Vec::new();
        };
        if let Some(buffer) = self.pair_ref(pair) {
            return vec![buffer];
        }
        let Expression::Identifier(pair) = pair.without_parentheses() else {
            return Vec::new();
        };
        let Some(symbol) = self.symbol(pair) else {
            return Vec::new();
        };
        let defs: Vec<Def<'a>> = self.defs(symbol).into_iter().filter(|def| def.end <= call.span.start).collect();
        let Some(last) = defs.iter().max_by_key(|def| def.end) else {
            return Vec::new();
        };
        if defs.iter().any(|def| def.end != last.end && self.exclusive(def.node, last.start)) {
            return Vec::new();
        }
        if let Some(buffer) = last.rhs.and_then(|rhs| self.pair_ref(rhs)) {
            return vec![buffer];
        }
        let short = matches!(last.rhs.map(Expression::without_parentheses),
            Some(Expression::ArrayExpression(array)) if array.elements.len() <= 1
                && array.elements.iter().all(|element| element.as_expression().is_some()));
        if !short {
            return Vec::new();
        }
        // `pair[1] = v` stores between the short literal and the emission.
        let mut stored = Vec::new();
        for reference in self.scoping().get_resolved_references(symbol) {
            let id = reference.node_id();
            let start = self.nodes().kind(id).span().start;
            if start < last.end || start >= call.span.start {
                continue;
            }
            let AstKind::ComputedMemberExpression(member) = self.nodes().parent_kind(id) else {
                continue;
            };
            if !matches!(member.expression.without_parentheses(),
                Expression::NumericLiteral(index) if index.value == 1.0)
            {
                continue;
            }
            let member_id = self.nodes().parent_id(id);
            let value = self.nodes().ancestor_ids(member_id).take(2).find_map(|ancestor| match self.nodes().kind(ancestor) {
                AstKind::AssignmentExpression(assignment) if assignment.left.span() == member.span => Some(&assignment.right),
                _ => None,
            });
            let Some(value) = value else {
                continue;
            };
            let value = match value.without_parentheses() {
                Expression::CallExpression(store) => {
                    match crate::signals::extract::layout::inline_store_returned(store, self.scoping()) {
                        Some(returned) => returned,
                        None => return Vec::new(),
                    }
                }
                _ => value,
            };
            match self.buffer_ref(value) {
                Some(buffer) => stored.push(buffer),
                None => return Vec::new(),
            }
        }
        match stored.as_slice() {
            [buffer] => vec![*buffer],
            _ => Vec::new(),
        }
    }

    /// `[aux, buf]` with `buf` a binding or read out of a register tuple.
    fn pair_ref(&self, pair: &'a Expression<'a>) -> Option<BufferRef> {
        let Expression::ArrayExpression(pair) = pair.without_parentheses() else {
            return None;
        };
        if pair.elements.len() != 2 {
            return None;
        }
        if matches!(pair.elements[0], ArrayExpressionElement::SpreadElement(_) | ArrayExpressionElement::Elision(_)) {
            return None;
        }
        let buffer = pair.elements[1].as_expression()?;
        if !matches!(buffer.without_parentheses(), Expression::ComputedMemberExpression(_)) {
            return None;
        }
        self.buffer_ref(buffer)
    }

    fn slot(&self, view: &SlotView) -> SlotInfo {
        let mut found: Vec<(Option<LifecycleRole>, Vec<(bool, &'a Expression<'a>, u32)>, bool)> = Vec::new();
        let mut seen_sites = HashSet::new();
        for anchor in view.anchors() {
            for id in self.nodes_at(*anchor) {
                let AstKind::CallExpression(call) = self.nodes().kind(id) else {
                    continue;
                };
                let buffers = self.emission_refs(call);
                if buffers.is_empty() || !seen_sites.insert(call.span.start) {
                    continue;
                }
                let Some(function) = self.enclosing_function(id) else {
                    continue;
                };
                // A register-packed buffer (an empty literal held in a tuple
                // register) has no binding: only its packed writes count.
                if let [BufferRef::Literal(start, end)] = buffers.as_slice() {
                    let phase = view
                        .callback_spans()
                        .iter()
                        .filter(|(_, span)| span.start <= call.span.start && call.span.start < span.end)
                        .min_by_key(|(_, span)| span.end - span.start)
                        .map(|(role, _)| *role);
                    let mut writes = Vec::new();
                    self.packed_writes(function, BufferRef::Literal(*start, *end), *end, 4, call.span.start, &mut writes);
                    writes.sort_by_key(|(_, _, _, end)| *end);
                    writes.dedup_by_key(|(_, _, start, end)| (*start, *end));
                    found.push((phase, writes.into_iter().map(|(byte, value, start, _)| (byte, value, start)).collect(), true));
                    continue;
                }
                let symbols: Vec<SymbolId> = buffers
                    .iter()
                    .filter_map(|buffer| match buffer {
                        BufferRef::Binding(symbol) => Some(*symbol),
                        BufferRef::Literal(..) => None,
                    })
                    .collect();
                if symbols.len() != buffers.len() {
                    continue;
                }
                let phase = view
                    .callback_spans()
                    .iter()
                    .filter(|(_, span)| span.start <= call.span.start && call.span.start < span.end)
                    .min_by_key(|(_, span)| span.end - span.start)
                    .map(|(role, _)| *role);
                let mut symbols = symbols;
                for symbol in symbols.clone() {
                    for other in self.arm_swapped_names(symbol, call.span.start) {
                        if !symbols.contains(&other) {
                            symbols.push(other);
                        }
                    }
                }
                let mut packed = false;
                let mut writes: Vec<(bool, &'a Expression<'a>, u32, u32)> = symbols
                    .iter()
                    .flat_map(|symbol| self.writes(function, *symbol, call.span.start, &mut packed))
                    .collect();
                for buffer in symbols.clone() {
                    for alias in self.copy_names(buffer, call.span.start) {
                        if symbols.contains(&alias) {
                            continue;
                        }
                        writes.extend(
                            self.writes(function, alias, call.span.start, &mut packed)
                                .into_iter()
                                .filter(|(_, _, start, _)| self.holds_copy(alias, buffer, *start)),
                        );
                    }
                }
                writes.sort_by_key(|(_, _, _, end)| *end);
                writes.dedup_by_key(|(_, _, start, end)| (*start, *end));
                found.push((phase, writes.into_iter().map(|(byte, value, start, _)| (byte, value, start)).collect(), packed));
            }
        }
        let mut info = SlotInfo::default();
        let mut store_sites = HashSet::new();
        for emission in view.emissions() {
            let ops: Vec<bool> = emission.leaves.iter().map(|leaf| leaf.op == LeafOp::Byte).collect();
            let candidates: Vec<&(Option<LifecycleRole>, Vec<(bool, &'a Expression<'a>, u32)>, bool)> = found
                .iter()
                .filter(|(phase, writes, _)| {
                    *phase == emission.phase
                        && writes.iter().map(|(byte, _, _)| *byte).collect::<Vec<_>>() == ops
                })
                .collect();
            let mut computed: Vec<EmissionInfo> = Vec::new();
            for (_, writes, packed) in &candidates {
                let mut out = EmissionInfo::default();
                for (byte, value, at) in writes {
                    let write = self.write_info(*byte, value, *at, *packed, &mut info.stores, &mut store_sites);
                    info.facts.extend(&write.facts);
                    if let Some(padded) = &write.padded {
                        info.facts.extend(padded);
                    }
                    out.writes.push(write);
                }
                computed.push(out);
            }
            // Every AST emission with the same phase and op sequence is a
            // candidate (duplicated tails); a binder uses them only when
            // they agree on the names they yield.
            computed.dedup_by(|a, b| same_writes(a, b));
            info.emissions.push(computed);
        }
        info
    }

    /// Names bound to `buffer` in one arm of an `if` while `buffer` is
    /// bound to them in the other (`if (g) { a = b; } else { b = a; }`):
    /// after the `if` both hold the emitted array.
    fn arm_swapped_names(&self, buffer: SymbolId, before: u32) -> Vec<SymbolId> {
        let copies = |from: SymbolId| -> Vec<(SymbolId, Def<'a>)> {
            self.defs(from)
                .into_iter()
                .filter(|def| def.end <= before)
                .filter_map(|def| match def.rhs.map(Expression::without_parentheses) {
                    Some(Expression::Identifier(source)) => self.symbol(source).map(|source| (source, def)),
                    _ => None,
                })
                .collect()
        };
        let into_buffer = copies(buffer);
        let mut out = Vec::new();
        // `sent = buf; buf[6] = v; emit(sent)`: the latest definition of
        // the sent name before the emission copies another buffer name.
        // `if (g) { …; sent = a; } else { …; sent = b; }`: each arm's copy.
        // `sent = buf; buf[6] = v; sent = [[], sent]; emit(sent)`: the
        // re-binding as the sent pair keeps the array the copy named.
        let defs: Vec<Def<'a>> = self
            .defs(buffer)
            .into_iter()
            .filter(|def| def.end <= before)
            .filter(|def| {
                def.rhs
                    .and_then(pair_buffer)
                    .and_then(|element| self.symbol(element))
                    != Some(buffer)
            })
            .collect();
        if let Some(latest) = defs.iter().max_by_key(|def| def.end) {
            for def in defs.iter().filter(|def| def.end == latest.end || self.exclusive(def.node, latest.start)) {
                if let Some(Expression::Identifier(source)) = def.rhs.map(Expression::without_parentheses) {
                    if let Some(source) = self.symbol(source).filter(|source| *source != buffer && !out.contains(source)) {
                        out.push(source);
                    }
                }
            }
        }
        for (other, def) in &into_buffer {
            if *other == buffer || out.contains(other) {
                continue;
            }
            let back = copies(*other)
                .into_iter()
                .any(|(source, reverse)| source == buffer && self.exclusive(reverse.node, def.start));
            if back {
                out.push(*other);
            }
        }
        out
    }

    /// Names assigned a copy of `buffer` before the emission at `before`
    /// (`if (g) { …; a = buf; } else { …; a = buf; } a[6] = v ^ ks();
    /// emit(buf)`). Their stores count only where [`Self::holds_copy`].
    fn copy_names(&self, buffer: SymbolId, before: u32) -> Vec<SymbolId> {
        let mut out = Vec::new();
        for reference in self.scoping().get_resolved_references(buffer) {
            let id = reference.node_id();
            if self.nodes().kind(id).span().start >= before {
                continue;
            }
            let AstKind::AssignmentExpression(assignment) = self.nodes().parent_kind(id) else {
                continue;
            };
            if assignment.right.span() != self.nodes().kind(id).span() {
                continue;
            }
            let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else {
                continue;
            };
            let Some(other) = self.symbol(target) else {
                continue;
            };
            if other == buffer || out.contains(&other) {
                continue;
            }
            out.push(other);
        }
        out
    }

    /// At `at`, `alias` holds `buffer`: its latest definition before `at`
    /// and every definition in the other arm of an `if` with it are
    /// `alias = buffer` (a reused register holds something else).
    fn holds_copy(&self, alias: SymbolId, buffer: SymbolId, at: u32) -> bool {
        let defs: Vec<Def<'a>> = self.defs(alias).into_iter().filter(|def| def.end <= at).collect();
        let Some(last) = defs.iter().max_by_key(|def| def.end) else {
            return false;
        };
        defs.iter()
            .filter(|def| def.end == last.end || self.exclusive(def.node, last.start))
            .all(|def| {
                matches!(def.rhs.map(Expression::without_parentheses),
                    Some(Expression::Identifier(source)) if self.symbol(source) == Some(buffer))
            })
    }

    /// Writes into `buffer` inside `function` before the emission at `before`.
    fn writes(&self, function: NodeId, buffer: SymbolId, before: u32, packed: &mut bool) -> Vec<(bool, &'a Expression<'a>, u32, u32)> {
        let mut out: Vec<(bool, &'a Expression<'a>, u32, u32)> = Vec::new();
        // A buffer register may be reused: only writes after its latest
        // array (raw prefix) definition before the emission count. When
        // that definition sits in one arm of an `if`, the other arm's
        // definition starts the same message.
        let prefixes: Vec<Def<'a>> = self
            .defs(buffer)
            .into_iter()
            .filter(|def| def.end <= before)
            .filter(|def| def.rhs.is_some_and(|rhs| matches!(rhs.without_parentheses(), Expression::ArrayExpression(_))))
            // `buf = [[], buf]` re-binds the buffer as the sent pair; it
            // is not a raw prefix.
            .filter(|def| def.rhs.is_none_or(|rhs| pair_buffer(rhs).is_none()))
            .collect();
        let since = match prefixes.iter().max_by_key(|def| def.end) {
            Some(last) => prefixes
                .iter()
                .filter(|def| def.end == last.end || self.exclusive(def.node, last.start))
                .map(|def| def.end)
                .min()
                .unwrap_or(last.end),
            None => 0,
        };
        // A short prefix literal in any arm that starts the message
        // (`if (g) { b = [s >>> 24]; b[1] = …; } else { b = [ …4 ] }`)
        // makes stores below index 4 prefix bytes.
        let prefix_len = prefixes
            .iter()
            .filter(|def| def.end >= since)
            .any(|def| matches!(def.rhs.map(Expression::without_parentheses),
                Some(Expression::ArrayExpression(array)) if array.elements.len() < 4))
            .then_some(4)
            .unwrap_or(0);
        // `[s >>> 24, …, s & 255, v ^ ks()]`: elements after the four
        // prefix bytes of the latest allocation are byte writes.
        if let Some(last) = prefixes.iter().max_by_key(|def| def.end) {
            if let Some(Expression::ArrayExpression(array)) = last.rhs.map(Expression::without_parentheses) {
                for element in array.elements.iter().skip(4) {
                    if let Some(value) = element.as_expression() {
                        out.push((true, value, value.span().start, value.span().end));
                    }
                }
            }
        }
        for reference in self.scoping().get_resolved_references(buffer) {
            let id = reference.node_id();
            let span = self.nodes().kind(id).span();
            if span.start >= before
                || span.start < since
                || !self.nodes().ancestor_ids(id).any(|ancestor| ancestor == function)
            {
                continue;
            }
            match self.nodes().parent_kind(id) {
                AstKind::ComputedMemberExpression(member) if member.object.span() == span => {
                    let member_id = self.nodes().parent_id(id);
                    let assignment = self.nodes().ancestor_ids(member_id).take(2).find_map(|ancestor| {
                        match self.nodes().kind(ancestor) {
                            AstKind::AssignmentExpression(assignment)
                                if assignment.left.span() == member.span =>
                            {
                                Some(assignment)
                            }
                            _ => None,
                        }
                    });
                    // `buf = [s >>> 24]; buf[1] = s >> 16 & 255; …`: stores
                    // completing a short prefix literal are prefix bytes.
                    let prefix_store = matches!(member.expression.without_parentheses(),
                        Expression::NumericLiteral(index) if index.value < prefix_len as f64);
                    if let Some(assignment) = assignment.filter(|_| !prefix_store) {
                        out.push((true, &assignment.right, assignment.span.start, assignment.span.end));
                    }
                }
                // `writer(mask, value, cond ? buf : other, pos)`: the
                // buffer argument chosen by a conditional is still a write.
                AstKind::ConditionalExpression(conditional)
                    if conditional.consequent.span() == span || conditional.alternate.span() == span =>
                {
                    let conditional_id = self.nodes().parent_id(id);
                    if let AstKind::CallExpression(call) = self.nodes().parent_kind(conditional_id) {
                        if emitted_buffer(call).is_none() {
                            let position = call
                                .arguments
                                .iter()
                                .position(|argument| argument.span() == conditional.span);
                            if let Some(position) = position.filter(|position| *position > 0) {
                                if let Some(value) = call.arguments[position - 1].as_expression() {
                                    out.push((false, value, call.span.start, call.span.end));
                                }
                            }
                        }
                    }
                }
                // `t = [writer, value, buf]; t[0](t[1], t[2], pos)`: the
                // tracer's argument tuple holds the buffer.
                AstKind::ArrayExpression(array) => {
                    out.extend(self.tuple_writes(id, array, before));
                }
                AstKind::CallExpression(call) if emitted_buffer(call).is_none() => {
                    let position = call
                        .arguments
                        .iter()
                        .position(|argument| argument.span() == span);
                    if let Some(position) = position {
                        if let Some(value) = inline_helper_value(call, position).or_else(|| self.named_helper_value(call, position)) {
                            out.push((false, value, call.span.start, call.span.end));
                        } else if position > 0 && !is_inline_function(&call.callee) {
                            if let Some(value) = call.arguments[position - 1].as_expression() {
                                out.push((false, value, call.span.start, call.span.end));
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        // Stores and writer calls reaching the buffer through register
        // tuples (`p = [t[0], i]; p[0][p[1]] = v`) that the reference walk
        // above does not see.
        let mut extra = Vec::new();
        self.packed_writes(function, BufferRef::Binding(buffer), since, prefix_len, before, &mut extra);
        for write in extra {
            if !out.iter().any(|(_, _, start, end)| (*start, *end) == (write.2, write.3)) {
                *packed = true;
                out.push(write);
            }
        }
        // Evaluation order: a write completes when its store (or writer
        // call) ends; a writer call inside a byte store's index runs first.
        out.sort_by_key(|(_, _, _, end)| *end);
        out
    }

    /// [`inline_helper_value`] for a helper the tracer outlined as a local
    /// function declaration (`function h(w, m, hi, lo, pad, p, b) { return
    /// w(m, hi + (lo ^ pad), b, p); }`) bound once and never reassigned.
    fn named_helper_value(&self, call: &'a CallExpression<'a>, position: usize) -> Option<&'a Expression<'a>> {
        let Expression::Identifier(callee) = call.callee.without_parentheses() else {
            return None;
        };
        let symbol = self.symbol(callee)?;
        if self.scoping().get_resolved_references(symbol).any(|reference| reference.is_write()) {
            return None;
        }
        let AstKind::Function(function) = self.nodes().kind(self.scoping().symbol_declaration(symbol)) else {
            return None;
        };
        if function.r#async || function.generator {
            return None;
        }
        helper_body_value(&function.params, &function.body.as_ref()?.statements, call, position)
    }

    /// `t[k]` (read at its own offset) when `t`'s only reaching definition
    /// in the reader's function is an array literal and nothing stores into
    /// `t` or lets it escape between that definition and the read: element
    /// `k`, followed through further tuples. The tracer packs registers this
    /// way (`t = [buf, i, ks()]; t[0][t[1]] = t[2] ^ c`).
    fn tuple_element(&self, expression: &Expression<'a>, depth: usize) -> Option<&'a Expression<'a>> {
        let Expression::ComputedMemberExpression(member) = expression.without_parentheses() else {
            return None;
        };
        let Expression::Identifier(object) = member.object.without_parentheses() else {
            return None;
        };
        let Expression::NumericLiteral(index) = member.expression.without_parentheses() else {
            return None;
        };
        self.tuple_element_at(object, index.value, member.span.start, depth)
    }

    fn tuple_element_at(&self, object: &IdentifierReference<'a>, index: f64, read: u32, depth: usize) -> Option<&'a Expression<'a>> {
        if depth == 0 || index < 0.0 || index.fract() != 0.0 {
            return None;
        }
        let symbol = self.symbol(object)?;
        let function = self.enclosing_function(object.node_id.get());
        let mut defs = self.reaching(symbol, read, read, function);
        if defs.len() != 1 {
            return None;
        }
        let def = defs.remove(0);
        if def.end > read || def.function != function {
            return None;
        }
        let Expression::ArrayExpression(array) = def.rhs?.without_parentheses() else {
            return None;
        };
        if array.elements.len() < 2 {
            return None;
        }
        let element = array.elements.get(index as usize)?.as_expression()?;
        // Only element reads `t[<literal>]` between the definition and the
        // read: a store into `t`, or `t` itself passed on, may replace it.
        for reference in self.scoping().get_resolved_references(symbol) {
            let id = reference.node_id();
            let at = self.nodes().kind(id).span().start;
            if at < def.end || at >= read {
                continue;
            }
            let element_read = match self.nodes().parent_kind(id) {
                AstKind::ComputedMemberExpression(outer) => {
                    matches!(outer.expression.without_parentheses(), Expression::NumericLiteral(_))
                        && outer.object.span() == self.nodes().kind(id).span()
                        && !self.is_store_target(self.nodes().parent_id(id))
                }
                _ => false,
            };
            if !element_read && !reference.is_write() {
                return None;
            }
        }
        Some(match element.without_parentheses() {
            Expression::ComputedMemberExpression(_) => self.tuple_element(element, depth - 1).unwrap_or(element),
            _ => element,
        })
    }

    /// The member expression `id` is the target of an assignment or update.
    fn is_store_target(&self, id: NodeId) -> bool {
        let span = self.nodes().kind(id).span();
        self.nodes().ancestor_ids(id).take(3).any(|ancestor| match self.nodes().kind(ancestor) {
            AstKind::AssignmentExpression(assignment) => assignment.left.span() == span,
            AstKind::UpdateExpression(update) => update.argument.span() == span,
            _ => false,
        })
    }

    /// The buffer an expression names: a binding, or one read out of a
    /// register tuple (a binding element or an empty literal element).
    fn buffer_ref(&self, expression: &Expression<'a>) -> Option<BufferRef> {
        match expression.without_parentheses() {
            Expression::Identifier(identifier) => self.symbol(identifier).map(BufferRef::Binding),
            // An inline register tuple (`[k, buf][1][i] = v`).
            Expression::ComputedMemberExpression(member)
                if matches!(member.object.without_parentheses(), Expression::ArrayExpression(_)) =>
            {
                let Expression::ArrayExpression(array) = member.object.without_parentheses() else { return None };
                let Expression::NumericLiteral(index) = member.expression.without_parentheses() else { return None };
                if index.value < 0.0 || index.value.fract() != 0.0 {
                    return None;
                }
                match array.elements.get(index.value as usize)?.as_expression()?.without_parentheses() {
                    Expression::Identifier(identifier) => self.symbol(identifier).map(BufferRef::Binding),
                    _ => None,
                }
            }
            Expression::ComputedMemberExpression(_) => match self.tuple_element(expression, TUPLE_DEPTH)?.without_parentheses() {
                Expression::Identifier(identifier) => self.symbol(identifier).map(BufferRef::Binding),
                Expression::ArrayExpression(array) if array.elements.is_empty() => {
                    Some(BufferRef::Literal(array.span.start, array.span.end))
                }
                _ => None,
            },
            _ => None,
        }
    }

    /// The written value of a packed store: `t[j] ^ c` with `t[j]` a
    /// keystream draw is `c`; a value read out of a tuple is its element.
    fn packed_value(&self, value: &'a Expression<'a>) -> &'a Expression<'a> {
        let resolve = |expression: &'a Expression<'a>| self.tuple_element(expression, TUPLE_DEPTH).unwrap_or(expression);
        let draw = |expression: &'a Expression<'a>| {
            matches!(resolve(expression).without_parentheses(), Expression::CallExpression(call) if call.arguments.is_empty())
        };
        if let Expression::BinaryExpression(binary) = value.without_parentheses() {
            if binary.operator == BinaryOperator::BitwiseXOR {
                if draw(&binary.left) && !draw(&binary.right) {
                    return resolve(&binary.right);
                }
                if draw(&binary.right) && !draw(&binary.left) {
                    return resolve(&binary.left);
                }
            }
        }
        resolve(value)
    }

    /// Writes into `target` inside `function` between `since` and `before`
    /// that go through a register tuple: `t[k][i] = v` and `w(…, v, t[k],
    /// pos)` with `t[k]` holding the buffer. Stores at a literal index below
    /// `prefix_len` are raw prefix bytes.
    fn packed_writes(
        &self,
        function: NodeId,
        target: BufferRef,
        since: u32,
        prefix_len: usize,
        before: u32,
        out: &mut Vec<(bool, &'a Expression<'a>, u32, u32)>,
    ) {
        let function_span = self.nodes().kind(function).span();
        let first = self.starts.partition_point(|(start, _)| *start < since.max(function_span.start));
        for (start, id) in &self.starts[first..] {
            if *start >= before || *start >= function_span.end {
                break;
            }
            let id = NodeId::new(*id as usize);
            match self.nodes().kind(id) {
                AstKind::AssignmentExpression(assignment) => {
                    let AssignmentTarget::ComputedMemberExpression(store) = &assignment.left else {
                        continue;
                    };
                    if assignment.span.end > before
                        || !matches!(store.object.without_parentheses(), Expression::ComputedMemberExpression(_))
                        || self.buffer_ref(&store.object) != Some(target)
                    {
                        continue;
                    }
                    if matches!(store.expression.without_parentheses(),
                        Expression::NumericLiteral(index) if index.value < prefix_len as f64)
                    {
                        continue;
                    }
                    out.push((true, self.packed_value(&assignment.right), assignment.span.start, assignment.span.end));
                }
                AstKind::CallExpression(call) => {
                    if call.span.end > before || emitted_buffer(call).is_some() {
                        continue;
                    }
                    let position = call.arguments.iter().position(|argument| {
                        argument.as_expression().is_some_and(|expression| {
                            matches!(expression.without_parentheses(), Expression::ComputedMemberExpression(_))
                                && self.buffer_ref(expression) == Some(target)
                        })
                    });
                    let Some(position) = position.filter(|position| *position > 0) else {
                        continue;
                    };
                    let value = if is_inline_function(&call.callee) {
                        inline_helper_value(call, position)
                    } else {
                        call.arguments[position - 1].as_expression()
                    };
                    if let Some(value) = value {
                        let value = self.tuple_element(value, TUPLE_DEPTH).unwrap_or(value);
                        out.push((false, value, call.span.start, call.span.end));
                    }
                }
                _ => {}
            }
        }
    }

    /// Writes through a tuple literal holding the buffer (element `id`):
    /// every call before `before` passing `t[k]` (the buffer's element)
    /// writes the argument before it, read back through the tuple.
    fn tuple_writes(&self, id: NodeId, array: &'a ArrayExpression<'a>, before: u32) -> Vec<(bool, &'a Expression<'a>, u32, u32)> {
        let mut out = Vec::new();
        let span = self.nodes().kind(id).span();
        let Some(slot) = array.elements.iter().position(|element| element.span() == span) else {
            return out;
        };
        let array_id = self.nodes().parent_id(id);
        let tuple = match self.nodes().parent_kind(array_id) {
            AstKind::VariableDeclarator(declarator) => match &declarator.id {
                BindingPattern::BindingIdentifier(identifier) => identifier.symbol_id.get(),
                _ => None,
            },
            AstKind::AssignmentExpression(assignment) => match &assignment.left {
                AssignmentTarget::AssignmentTargetIdentifier(identifier) => self.symbol(identifier),
                _ => None,
            },
            _ => None,
        };
        let Some(tuple) = tuple else {
            return out;
        };
        let element_of = |expression: &Expression<'a>| -> Option<usize> {
            let Expression::ComputedMemberExpression(member) = expression.without_parentheses() else {
                return None;
            };
            let Expression::Identifier(object) = member.object.without_parentheses() else {
                return None;
            };
            if self.symbol(object) != Some(tuple) {
                return None;
            }
            match member.expression.without_parentheses() {
                Expression::NumericLiteral(index) if index.value.fract() == 0.0 && index.value >= 0.0 => Some(index.value as usize),
                _ => None,
            }
        };
        for reference in self.scoping().get_resolved_references(tuple) {
            let read = reference.node_id();
            if self.nodes().kind(read).span().start >= before || self.nodes().kind(read).span().start < array.span.end {
                continue;
            }
            let AstKind::ComputedMemberExpression(member) = self.nodes().parent_kind(read) else {
                continue;
            };
            let member_id = self.nodes().parent_id(read);
            // `t[k][t[i]] = v` with `t[k]` the buffer: a byte store; a
            // value `t[j] ^ c` with `t[j]` a keystream draw is the marker `c`.
            if let AstKind::ComputedMemberExpression(store) = self.nodes().parent_kind(member_id) {
                let store_id = self.nodes().parent_id(member_id);
                if store.object.span() == member.span && element_of(&store.object) == Some(slot) {
                    if let AstKind::AssignmentExpression(assignment) = self.nodes().parent_kind(store_id) {
                        if assignment.left.span() == store.span && assignment.span.end <= before {
                            let drawn = |side: &Expression<'a>| {
                                element_of(side)
                                    .and_then(|index| array.elements.get(index))
                                    .and_then(|element| element.as_expression())
                                    .is_some_and(|element| matches!(element.without_parentheses(),
                                        Expression::CallExpression(draw) if draw.arguments.is_empty()))
                            };
                            let value: &'a Expression<'a> = match assignment.right.without_parentheses() {
                                Expression::BinaryExpression(binary)
                                    if binary.operator == BinaryOperator::BitwiseXOR && drawn(&binary.left) =>
                                {
                                    &binary.right
                                }
                                Expression::BinaryExpression(binary)
                                    if binary.operator == BinaryOperator::BitwiseXOR && drawn(&binary.right) =>
                                {
                                    &binary.left
                                }
                                _ => &assignment.right,
                            };
                            out.push((true, value, assignment.span.start, assignment.span.end));
                        }
                    }
                }
                continue;
            }
            let AstKind::CallExpression(call) = self.nodes().parent_kind(member_id) else {
                continue;
            };
            let member_span = member.span;
            let Some(position) = call.arguments.iter().position(|argument| argument.span() == member_span) else {
                continue;
            };
            if position == 0 || call.arguments[position].as_expression().and_then(element_of) != Some(slot) {
                continue;
            }
            let Some(value) = call.arguments[position - 1].as_expression() else {
                continue;
            };
            let value = match element_of(value).and_then(|index| array.elements.get(index)) {
                Some(element) => match element.as_expression() {
                    Some(element) => element,
                    None => continue,
                },
                None => value,
            };
            out.push((false, value, call.span.start, call.span.end));
        }
        out
    }

    #[allow(clippy::too_many_arguments)]
    fn write_info(
        &self,
        byte: bool,
        value: &'a Expression<'a>,
        at: u32,
        packed: bool,
        stores: &mut Vec<Facts>,
        store_sites: &mut HashSet<u32>,
    ) -> WriteInfo {
        let value = if byte { unmask(value) } else { value };
        let value = value.without_parentheses();
        let literal = matches!(value, Expression::NumericLiteral(_));
        let mut facts = Facts::default();
        let mut budget = SLICE_BUDGET;
        let mut visited = HashSet::new();
        self.slice(value, at, SLICE_DEPTH, packed, &mut budget, &mut visited, &mut facts, stores, store_sites);
        let below_one = |test: &Expression<'a>| {
            matches!(test.without_parentheses(), Expression::BinaryExpression(test)
                if test.operator == BinaryOperator::LessThan
                    && matches!(test.right.without_parentheses(), Expression::NumericLiteral(one) if one.value == 1.0))
        };
        // `x < 1 ? x : …`, or the same choice written as an `if` whose
        // arms define the written binding.
        // The test may be bound first (`t = x < 1; w(t ? x : …)`).
        let below_one_test = |test: &'a Expression<'a>| {
            below_one(test)
                || (matches!(test.without_parentheses(), Expression::Identifier(_))
                    && self.single_def(test, at).and_then(|def| def.rhs).is_some_and(|rhs| below_one(rhs)))
        };
        let fraction = matches!(value, Expression::ConditionalExpression(conditional) if below_one_test(&conditional.test))
            || {
                let defs = self.reaching_defs(value, at);
                !defs.is_empty()
                    && defs.iter().all(|def| {
                        self.nodes().ancestor_ids(def.node).take(4).any(|ancestor| {
                            matches!(self.nodes().kind(ancestor), AstKind::IfStatement(statement) if below_one(&statement.test))
                        })
                    })
            };
        // `(k << 8) + (x & 255 ^ k)` written inline, or with its parts
        // bound to locals first (`a = k << 8; b = x & 255; w(a + (b ^ k))`).
        let operand = and_255_operand(value)
            .map(|operand| (operand, at))
            .or_else(|| self.padded_through_locals(value, at));
        let padded = operand.map(|(operand, from)| {
            let mut facts = Facts::default();
            let mut budget = SLICE_BUDGET;
            let mut visited = HashSet::new();
            let mut ignored = Vec::new();
            let mut ignored_sites = HashSet::new();
            self.slice(operand, from, SLICE_DEPTH, packed, &mut budget, &mut visited, &mut facts, &mut ignored, &mut ignored_sites);
            facts
        });
        let folded = operand.and_then(|(operand, from)| self.folded_literal(operand, from));
        WriteInfo {
            byte,
            literal,
            facts,
            fraction,
            padded,
            folded,
        }
    }

    /// The definitions of the binding `expression` names that reach `at`.
    fn reaching_defs(&self, expression: &'a Expression<'a>, at: u32) -> Vec<Def<'a>> {
        let Expression::Identifier(identifier) = expression.without_parentheses() else {
            return Vec::new();
        };
        let Some(symbol) = self.symbol(identifier) else {
            return Vec::new();
        };
        let function = self.enclosing_function(identifier.node_id.get());
        self.reaching(symbol, at, identifier.span.start, function)
    }

    /// The single definition of the binding `expression` names that
    /// reaches `at`.
    fn single_def(&self, expression: &'a Expression<'a>, at: u32) -> Option<Def<'a>> {
        let mut defs = self.reaching_defs(expression, at);
        (defs.len() == 1).then(|| defs.remove(0))
    }

    /// The `x` of a padded value whose `x & 255` part was bound to a local.
    fn padded_through_locals(&self, value: &'a Expression<'a>, at: u32) -> Option<(&'a Expression<'a>, u32)> {
        match value.without_parentheses() {
            Expression::BinaryExpression(binary) => self
                .padded_through_locals(&binary.left, at)
                .or_else(|| self.padded_through_locals(&binary.right, at)),
            expression @ Expression::Identifier(_) => {
                let def = self.single_def(expression, at)?;
                // `b = x & 255`, or the whole padded value bound first.
                and_255_operand(def.rhs?).map(|operand| (operand, def.start))
            }
            _ => None,
        }
    }

    /// A padded operand whose only reaching definition is a numeric
    /// literal (a measurement the tracer folded): its offset and value.
    fn folded_literal(&self, operand: &'a Expression<'a>, at: u32) -> Option<(u32, f64)> {
        let def = self.single_def(operand, at)?;
        match def.rhs?.without_parentheses() {
            Expression::NumericLiteral(literal) => Some((def.start, literal.value)),
            _ => None,
        }
    }

    /// Collect the facts of `expression` and follow every local binding it
    /// reads to its latest definition before `at`.
    #[allow(clippy::too_many_arguments)]
    fn slice(
        &self,
        expression: &'a Expression<'a>,
        at: u32,
        depth: usize,
        packed: bool,
        budget: &mut usize,
        visited: &mut HashSet<(SymbolId, u32)>,
        facts: &mut Facts,
        stores: &mut Vec<Facts>,
        store_sites: &mut HashSet<u32>,
    ) {
        let mut collector = FactCollector {
            walker: self,
            facts: Facts::default(),
            reads: Vec::new(),
            descend_functions: false,
            packed,
        };
        collector.visit_expression(expression);
        facts.extend(&collector.facts);
        if depth == 0 {
            return;
        }
        for (symbol, read_at, read_node) in collector.reads {
            let reader_function = self.enclosing_function(read_node);
            for def in self.reaching(symbol, read_at.min(at), read_at, reader_function) {
                if *budget == 0 || !visited.insert((symbol, def.start)) {
                    continue;
                }
                *budget -= 1;
                if let Some(rhs) = def.rhs {
                    if matches!(rhs.without_parentheses(), Expression::ArrayExpression(_)) {
                        self.element_stores(symbol, stores, store_sites);
                    }
                    self.slice(rhs, def.start, depth - 1, packed, budget, visited, facts, stores, store_sites);
                }
                let _ = def.end;
            }
        }
    }

    /// The definitions of `symbol` reaching a read at `at`: the textually
    /// latest one in the reader's function, else the latest earlier one,
    /// else (state written elsewhere) up to four definitions.
    fn reaching(&self, symbol: SymbolId, at: u32, read: u32, function: Option<NodeId>) -> Vec<Def<'a>> {
        let mut defs = self.defs(symbol);
        defs.sort_by_key(|def| def.start);
        let before: Vec<&Def<'a>> = defs
            .iter()
            .filter(|def| def.end <= at && !self.never_reaches(def.node, read))
            .collect();
        let pick = before
            .iter()
            .filter(|def| def.function == function)
            .max_by_key(|def| def.start)
            .or_else(|| before.iter().max_by_key(|def| def.start))
            .map(|def| def.start);
        match pick {
            // The other arm of an `if`/`?:` holding the picked definition
            // reaches the read too (`if (d) v = f(d); else v = null;`).
            Some(start) => {
                let arms: Vec<u32> = before
                    .iter()
                    .filter(|def| def.start != start && self.exclusive(def.node, start))
                    .map(|def| def.start)
                    .collect();
                defs.into_iter().filter(|def| def.start == start || arms.contains(&def.start)).collect()
            }
            None => defs.into_iter().take(4).collect(),
        }
    }

    /// `def` never reaches the read at `read`: it sits in one branch of an
    /// `if`/`?:` whose other branch holds the read, unless its branch ends
    /// in `continue` and a loop of the same function encloses the `if` (the
    /// definition of one iteration reaches the other branch's read in a
    /// later one:
    /// `for (;;) { if (i < n) { sum = sum + v; continue; } else { use(sum); } }`).
    fn never_reaches(&self, def: NodeId, read: u32) -> bool {
        let Some(branch) = self.exclusive_branch(def, read) else {
            return false;
        };
        // Only an arm that loops back (ends in `continue`) carries its
        // definition to the other arm's read, through the next iteration.
        let start = self.nodes().kind(def).span().start;
        let AstKind::IfStatement(statement) = self.nodes().kind(branch) else {
            return true;
        };
        let arm = if statement.consequent.span().start <= start && start < statement.consequent.span().end {
            Some(&statement.consequent)
        } else {
            statement.alternate.as_ref()
        };
        let loops_back = match arm {
            Some(Statement::ContinueStatement(_)) => true,
            Some(Statement::BlockStatement(block)) => matches!(block.body.last(), Some(Statement::ContinueStatement(_))),
            _ => false,
        };
        if !loops_back {
            return true;
        }
        for ancestor in self.nodes().ancestor_ids(branch) {
            match self.nodes().kind(ancestor) {
                AstKind::ForStatement(_)
                | AstKind::WhileStatement(_)
                | AstKind::DoWhileStatement(_)
                | AstKind::ForInStatement(_)
                | AstKind::ForOfStatement(_) => return false,
                AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) => return true,
                _ => {}
            }
        }
        true
    }

    /// `def` sits in one branch of an `if`/`?:` whose other branch holds
    /// the read at `read`.
    fn exclusive(&self, def: NodeId, read: u32) -> bool {
        self.exclusive_branch(def, read).is_some()
    }

    /// The `if`/`?:` of [`Self::exclusive`].
    fn exclusive_branch(&self, def: NodeId, read: u32) -> Option<NodeId> {
        let start = self.nodes().kind(def).span().start;
        let inside = |span: oxc_span::Span, offset: u32| span.start <= offset && offset < span.end;
        for ancestor in self.nodes().ancestor_ids(def) {
            let (consequent, alternate) = match self.nodes().kind(ancestor) {
                AstKind::IfStatement(statement) => {
                    (statement.consequent.span(), statement.alternate.as_ref().map(|a| a.span()))
                }
                AstKind::ConditionalExpression(conditional) => {
                    (conditional.consequent.span(), Some(conditional.alternate.span()))
                }
                AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) => return None,
                _ => continue,
            };
            let Some(alternate) = alternate else {
                continue;
            };
            if (inside(consequent, start) && inside(alternate, read))
                || (inside(alternate, start) && inside(consequent, read))
            {
                return Some(ancestor);
            }
        }
        None
    }

    fn defs(&self, symbol: SymbolId) -> Vec<Def<'a>> {
        let mut out = Vec::new();
        let declaration = self.scoping().symbol_declaration(symbol);
        if let AstKind::VariableDeclarator(declarator) = self.nodes().kind(declaration) {
            out.push(Def {
                node: declaration,
                start: declarator.span.start,
                end: declarator.span.end,
                function: self.enclosing_function(declaration),
                rhs: declarator.init.as_ref(),
            });
        }
        for reference in self.scoping().get_resolved_references(symbol) {
            if !reference.is_write() {
                continue;
            }
            let id = reference.node_id();
            let start = self.nodes().kind(id).span().start;
            for ancestor in self.nodes().ancestor_ids(id).take(4) {
                match self.nodes().kind(ancestor) {
                    AstKind::AssignmentExpression(assignment) if assignment.left.span().start == start => {
                        out.push(Def {
                            node: ancestor,
                            start: assignment.span.start,
                            end: assignment.span.end,
                            function: self.enclosing_function(ancestor),
                            rhs: Some(&assignment.right),
                        });
                        break;
                    }
                    AstKind::UpdateExpression(update) => {
                        out.push(Def {
                            node: ancestor,
                            start: update.span.start,
                            end: update.span.end,
                            function: self.enclosing_function(ancestor),
                            rhs: None,
                        });
                        break;
                    }
                    _ => {}
                }
            }
        }
        out
    }

    /// Contexts of `array[k] = …` stores of a flag array.
    fn element_stores(&self, symbol: SymbolId, stores: &mut Vec<Facts>, sites: &mut HashSet<u32>) {
        for reference in self.scoping().get_resolved_references(symbol) {
            let id = reference.node_id();
            let AstKind::ComputedMemberExpression(member) = self.nodes().parent_kind(id) else {
                continue;
            };
            let member_id = self.nodes().parent_id(id);
            let assignment = self.nodes().ancestor_ids(member_id).take(2).find_map(|ancestor| {
                match self.nodes().kind(ancestor) {
                    AstKind::AssignmentExpression(assignment)
                        if assignment.left.span() == member.span =>
                    {
                        Some(ancestor)
                    }
                    _ => None,
                }
            });
            let Some(assignment) = assignment else {
                continue;
            };
            if !sites.insert(member.span.start) {
                continue;
            }
            stores.push(self.store_context(assignment));
        }
    }

    /// The enclosing `try` of a store (crossing one getter function), else
    /// its outermost enclosing `if` inside the same function and loop body.
    fn store_context(&self, store: NodeId) -> Facts {
        let mut first_if: Option<NodeId> = None;
        let mut crossed_function = false;
        for ancestor in self.nodes().ancestor_ids(store) {
            match self.nodes().kind(ancestor) {
                AstKind::TryStatement(_) => return self.facts_of(ancestor),
                AstKind::IfStatement(_) if !crossed_function => {
                    // A loop's exit test (the tracer nests the rest of a
                    // collector in it) is not the test around this store.
                    if first_if.is_some() && self.exits_loop(ancestor) {
                        break;
                    }
                    first_if = Some(ancestor);
                }
                AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) => {
                    // One getter boundary at most: a store two functions
                    // deep has no own `try`, and a try around the whole
                    // collector says nothing about this store.
                    if first_if.is_some() || crossed_function {
                        break;
                    }
                    crossed_function = true;
                }
                // A loop body is its own region: the tracer may nest the
                // rest of a collector in the loop it un-nested, so an `if`
                // around the loop is not this store's test.
                AstKind::WhileStatement(_)
                | AstKind::DoWhileStatement(_)
                | AstKind::ForStatement(_)
                | AstKind::ForInStatement(_)
                | AstKind::ForOfStatement(_)
                    if first_if.is_some() =>
                {
                    break
                }
                AstKind::Program(_) => break,
                _ => {}
            }
        }
        first_if.map(|id| self.facts_of(id)).unwrap_or_default()
    }

    /// `statement` is an `if` inside a loop (in its function) whose
    /// consequent ends by leaving the loop (`return` / `break`): the loop's
    /// exit, under which the tracer nests the rest of a collector.
    fn exits_loop(&self, statement: NodeId) -> bool {
        let AstKind::IfStatement(statement_if) = self.nodes().kind(statement) else {
            return false;
        };
        let last = match &statement_if.consequent {
            Statement::BlockStatement(block) => block.body.last(),
            other => Some(other),
        };
        if !matches!(last, Some(Statement::ReturnStatement(_) | Statement::BreakStatement(_))) {
            return false;
        }
        for ancestor in self.nodes().ancestor_ids(statement) {
            match self.nodes().kind(ancestor) {
                AstKind::WhileStatement(_)
                | AstKind::DoWhileStatement(_)
                | AstKind::ForStatement(_)
                | AstKind::ForInStatement(_)
                | AstKind::ForOfStatement(_) => return true,
                AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) | AstKind::Program(_) => return false,
                _ => {}
            }
        }
        false
    }

    fn facts_of(&self, id: NodeId) -> Facts {
        let mut collector = FactCollector {
            walker: self,
            facts: Facts::default(),
            reads: Vec::new(),
            descend_functions: true,
            packed: false,
        };
        match self.nodes().kind(id) {
            AstKind::TryStatement(statement) => collector.visit_try_statement(statement),
            AstKind::IfStatement(statement) => collector.visit_if_statement(statement),
            _ => {}
        }
        collector.facts
    }
}

fn same_writes(left: &EmissionInfo, right: &EmissionInfo) -> bool {
    left.writes.len() == right.writes.len()
        && left.writes.iter().zip(&right.writes).all(|(a, b)| {
            a.byte == b.byte && a.literal == b.literal && a.facts == b.facts && a.fraction == b.fraction && a.padded == b.padded && a.folded == b.folded
        })
}

fn is_inline_function(callee: &Expression<'_>) -> bool {
    matches!(
        callee.without_parentheses(),
        Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_)
    )
}

/// The written value of an inline helper the tracer left as an
/// immediately invoked function: `((w, m, hi, lo, pad, b, p) => { return
/// w(m, hi + (lo ^ pad), b, p); })(writer, mask, k << 8, x & 255, k, buf, pos)`
/// with `buf` at `position`. The body's single writer call passes the
/// buffer formal; its value argument reads formals, and the caller
/// argument bound to the formal holding `x & 255` (else the first formal
/// read) is the value.
fn inline_helper_value<'a>(call: &'a CallExpression<'a>, position: usize) -> Option<&'a Expression<'a>> {
    let (params, statements) = match call.callee.without_parentheses() {
        Expression::ArrowFunctionExpression(arrow) => (&arrow.params, &arrow.body.statements),
        Expression::FunctionExpression(function) => (&function.params, &function.body.as_ref()?.statements),
        _ => return None,
    };
    helper_body_value(params, statements, call, position)
}

/// [`inline_helper_value`] over a helper's formals and body.
fn helper_body_value<'a>(
    params: &'a FormalParameters<'a>,
    statements: &'a oxc_allocator::Vec<'a, Statement<'a>>,
    call: &'a CallExpression<'a>,
    position: usize,
) -> Option<&'a Expression<'a>> {
    let names: Vec<&str> = params
        .items
        .iter()
        .map(|param| match &param.pattern {
            BindingPattern::BindingIdentifier(identifier) => Some(identifier.name.as_str()),
            _ => None,
        })
        .collect::<Option<_>>()?;
    let buffer = *names.get(position)?;
    let [statement] = statements.as_slice() else {
        return None;
    };
    let body = match statement {
        Statement::ReturnStatement(statement) => statement.argument.as_ref()?,
        Statement::ExpressionStatement(statement) => &statement.expression,
        _ => return None,
    };
    let Expression::CallExpression(inner) = body.without_parentheses() else {
        return None;
    };
    let at = inner.arguments.iter().position(|argument| {
        matches!(argument.as_expression().map(Expression::without_parentheses), Some(Expression::Identifier(id)) if id.name == buffer)
    })?;
    let value = inner.arguments.get(at.checked_sub(1)?)?.as_expression()?;
    let mut read = Vec::new();
    formal_reads(value, &names, &mut read);
    let actual = |index: usize| call.arguments.get(index).and_then(|argument| argument.as_expression());
    read.iter()
        .copied()
        .find(|index| actual(*index).is_some_and(|argument| and_255_operand(argument).is_some()))
        .or_else(|| read.first().copied())
        .and_then(actual)
}

fn formal_reads(expression: &Expression<'_>, names: &[&str], out: &mut Vec<usize>) {
    match expression.without_parentheses() {
        Expression::Identifier(identifier) => {
            if let Some(index) = names.iter().position(|name| *name == identifier.name.as_str()) {
                out.push(index);
            }
        }
        Expression::BinaryExpression(binary) => {
            formal_reads(&binary.left, names, out);
            formal_reads(&binary.right, names, out);
        }
        Expression::UnaryExpression(unary) => formal_reads(&unary.argument, names, out),
        _ => {}
    }
}

/// `obs.next([[], buf])` → `buf`.
fn emitted_buffer<'a>(call: &'a CallExpression<'a>) -> Option<&'a IdentifierReference<'a>> {
    let Expression::StaticMemberExpression(callee) = call.callee.without_parentheses() else {
        return None;
    };
    if callee.property.name != "next" || call.arguments.len() != 1 {
        return None;
    }
    call.arguments[0].as_expression().and_then(pair_buffer)
}

/// `[[], buf]` → `buf`.
fn pair_buffer<'a>(pair: &'a Expression<'a>) -> Option<&'a IdentifierReference<'a>> {
    let Expression::ArrayExpression(pair) = pair.without_parentheses() else {
        return None;
    };
    if pair.elements.len() != 2 {
        return None;
    }
    // `[[], buf]` or `[list, buf]` (the first element is the message's
    // object list; the buffer is always second).
    match (&pair.elements[0], &pair.elements[1]) {
        (ArrayExpressionElement::SpreadElement(_) | ArrayExpressionElement::Elision(_), _) => None,
        (_, ArrayExpressionElement::Identifier(buffer)) => Some(buffer),
        _ => None,
    }
}

/// `v ^ ks()` / `ks() ^ v` → `v`.
fn unmask<'a>(value: &'a Expression<'a>) -> &'a Expression<'a> {
    if let Expression::BinaryExpression(binary) = value.without_parentheses() {
        if binary.operator == BinaryOperator::BitwiseXOR {
            let keystream = |expression: &Expression<'_>| {
                matches!(expression.without_parentheses(), Expression::CallExpression(call) if call.arguments.is_empty())
            };
            if keystream(&binary.right) && !keystream(&binary.left) {
                return &binary.left;
            }
            if keystream(&binary.left) && !keystream(&binary.right) {
                return &binary.right;
            }
        }
    }
    value
}

/// The `x` of a padded `… (x & 255 ^ pad)` value.
fn and_255_operand<'a>(value: &'a Expression<'a>) -> Option<&'a Expression<'a>> {
    match value.without_parentheses() {
        Expression::BinaryExpression(binary) => {
            if binary.operator == BinaryOperator::BitwiseAnd
                && matches!(binary.right.without_parentheses(), Expression::NumericLiteral(mask) if mask.value == 255.0)
            {
                return Some(&binary.left);
            }
            and_255_operand(&binary.left).or_else(|| and_255_operand(&binary.right))
        }
        _ => None,
    }
}

struct FactCollector<'v, 'w, 'c, 's, 'a> {
    walker: &'v Walker<'w, 'c, 's, 'a>,
    facts: Facts,
    /// Local reads: symbol, offset, node.
    reads: Vec<(SymbolId, u32, NodeId)>,
    descend_functions: bool,
    /// Read `t[k]` of a register tuple as its element (a register-packed
    /// emission, where a value is threaded through tuples).
    packed: bool,
}

impl<'v, 'w, 'c, 's, 'a> FactCollector<'v, 'w, 'c, 's, 'a> {
    /// A collector for a function literal's body (its reads are not followed).
    fn callback_collector(&self) -> FactCollector<'v, 'w, 'c, 's, 'a> {
        FactCollector {
            walker: self.walker,
            facts: Facts::default(),
            reads: Vec::new(),
            descend_functions: true,
            packed: self.packed,
        }
    }

    fn keep_callback(&mut self, inner: Facts) {
        self.facts.callback_regex.extend(inner.regex);
        self.facts.callback_regex.extend(inner.callback_regex);
        self.facts.callback_calls.extend(inner.calls);
        self.facts.callback_calls.extend(inner.callback_calls);
    }
}


impl<'a> Visit<'a> for FactCollector<'_, '_, '_, '_, 'a> {
    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        match self.walker.symbol(identifier) {
            Some(symbol) => self.reads.push((symbol, identifier.span.start, identifier.node_id.get())),
            None if !generated(&identifier.name) => {
                self.facts.globals.insert(identifier.name.to_string());
            }
            None => {}
        }
    }

    fn visit_static_member_expression(&mut self, member: &StaticMemberExpression<'a>) {
        let name = member.property.name.as_str();
        if !generated(name) {
            self.facts.members.insert(name.to_string());
            if name == "console" {
                if let Expression::Identifier(object) = member.object.without_parentheses() {
                    if self.walker.symbol(object).is_some_and(|symbol| self.walker.is_parameter(symbol)) {
                        self.facts.param_console = true;
                    }
                }
            }
        }
        walk::walk_static_member_expression(self, member);
    }

    fn visit_computed_member_expression(&mut self, member: &ComputedMemberExpression<'a>) {
        if self.packed {
            if let (Expression::Identifier(object), Expression::NumericLiteral(index)) =
                (member.object.without_parentheses(), member.expression.without_parentheses())
            {
                if let Some(element) = self.walker.tuple_element_at(object, index.value, member.span.start, TUPLE_DEPTH) {
                    self.visit_expression(element);
                    return;
                }
            }
        }
        if let Expression::StringLiteral(literal) = &member.expression {
            self.facts.members.insert(literal.value.to_string());
        }
        walk::walk_computed_member_expression(self, member);
    }

    fn visit_string_literal(&mut self, literal: &StringLiteral<'a>) {
        self.facts.strings.insert(literal.value.to_string());
    }

    fn visit_reg_exp_literal(&mut self, literal: &RegExpLiteral<'a>) {
        if let Some(raw) = &literal.raw {
            self.facts.regex.insert(raw.to_string());
        }
    }

    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        match call.callee.without_parentheses() {
            Expression::Identifier(identifier) => {
                if !generated(&identifier.name) {
                    self.facts.calls.insert(identifier.name.to_string());
                }
                if self.walker.symbol(identifier).is_none() && !generated(&identifier.name) {
                    self.facts.globals.insert(identifier.name.to_string());
                }
                // A local callee is a program function: not followed.
                call.arguments.iter().for_each(|argument| self.visit_argument(argument));
                return;
            }
            Expression::StaticMemberExpression(member) => {
                if !generated(&member.property.name) {
                    self.facts.calls.insert(member.property.name.to_string());
                }
                // `re.test(…)` of a binding only ever set to `RegExp("…")`
                // (the tracer hoists the pattern out of the callback).
                if matches!(member.property.name.as_str(), "test" | "exec") {
                    if let Expression::Identifier(object) = member.object.without_parentheses() {
                        if let Some(symbol) = self.walker.symbol(object) {
                            self.facts.regex.extend(self.walker.regexp_patterns(symbol));
                        }
                    }
                }
            }
            _ => {}
        }
        walk::walk_call_expression(self, call);
    }

    /// `[o, o.m]`: a bound-method pair, later invoked as
    /// `pair[1].call(pair[0], …)`; the method counts as called.
    fn visit_array_expression(&mut self, array: &ArrayExpression<'a>) {
        if let [ArrayExpressionElement::Identifier(object), ArrayExpressionElement::StaticMemberExpression(method)] =
            array.elements.as_slice()
        {
            if let Expression::Identifier(receiver) = method.object.without_parentheses() {
                if receiver.name == object.name && !generated(&method.property.name) {
                    self.facts.calls.insert(method.property.name.to_string());
                }
            }
        }
        walk::walk_array_expression(self, array);
    }

    fn visit_new_expression(&mut self, new: &NewExpression<'a>) {
        if let Expression::Identifier(identifier) = new.callee.without_parentheses() {
            if !generated(&identifier.name) {
                self.facts.calls.insert(identifier.name.to_string());
            }
        }
        walk::walk_new_expression(self, new);
    }

    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        if self.descend_functions {
            walk::walk_function(self, function, flags);
        } else {
            let mut inner = self.callback_collector();
            walk::walk_function(&mut inner, function, flags);
            self.keep_callback(inner.facts);
        }
    }

    fn visit_arrow_function_expression(&mut self, arrow: &ArrowFunctionExpression<'a>) {
        if self.descend_functions {
            walk::walk_arrow_function_expression(self, arrow);
        } else {
            let mut inner = self.callback_collector();
            walk::walk_arrow_function_expression(&mut inner, arrow);
            self.keep_callback(inner.facts);
        }
    }
}
