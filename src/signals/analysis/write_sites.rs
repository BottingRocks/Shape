//! Write-site access for binders and recognizers: from a wire leaf's
//! [`WriteSite`] (spans of the contract write node) to the parse.
//!
//! The leaf provenance of a [`super::SlotView`] is flow-insensitive: values
//! copied through shared registers carry every name the register ever held.
//! A binder that must tell fields apart by what the value at its own write
//! site is computed from reads the value expression here and walks the parse
//! itself ([`FinderCtx::parse`]). Only spans and AST structure are exposed;
//! generated spellings never are.

use std::collections::{BTreeMap, HashMap};

use oxc_ast::ast::IdentifierReference;
use oxc_ast::AstKind;
use oxc_semantic::{NodeId, SymbolId};
use oxc_span::{GetSpan, Span};

use super::{FinderCtx, LeafView, WriteSite};

/// Memo key of the span index.
#[derive(Hash, PartialEq, Eq)]
struct SpanIndexKey;

/// Innermost node of every exact span.
struct SpanIndex(HashMap<(u32, u32), NodeId>);

impl FinderCtx<'_, '_, '_> {
    fn span_index(&self) -> std::rc::Rc<SpanIndex> {
        self.memo(SpanIndexKey, || {
            let mut map = HashMap::new();
            // Nodes are enumerated parent before child: the last node seen
            // with a span is the innermost one (`Argument` -> expression).
            for node in self.parse.nodes.iter() {
                let span = node.kind().span();
                map.insert((span.start, span.end), node.id());
            }
            SpanIndex(map)
        })
    }

    /// The innermost AST node whose span is exactly `span`.
    pub(crate) fn node_with_span(&self, span: Span) -> Option<NodeId> {
        self.span_index().0.get(&(span.start, span.end)).copied()
    }

    /// The value expression written at `write` (inside the helper for a
    /// helper write).
    pub(crate) fn write_value_node(&self, write: &WriteSite) -> Option<NodeId> {
        self.node_with_span(write.value_span)
    }

    /// The caller-side value of `leaf`: for a helper write whose value is a
    /// helper formal, the argument the caller passes for it; otherwise
    /// [`Self::value_node`].
    pub(crate) fn caller_value_node(&self, leaf: &LeafView) -> Option<NodeId> {
        self.caller_write_value_node(&leaf.write)
    }

    /// [`Self::caller_value_node`] of a bare write site.
    pub(crate) fn caller_write_value_node(&self, write: &WriteSite) -> Option<NodeId> {
        let value = self.write_value_node(write)?;
        if write.helper_call.is_none() {
            return Some(self.through_tuple(value));
        }
        let formal = self.reference_symbol(value).and_then(|symbol| self.formal_index(symbol));
        match formal.and_then(|index| write.helper_arguments.get(index)) {
            Some(argument) => self.node_with_span(Span::new(argument.start, argument.end)).map(|node| self.through_tuple(node)),
            None => Some(self.through_tuple(value)),
        }
    }

    /// A helper write whose value is `record[key]` with both helper formals
    /// (`((w, m, r, k, b, p) => w(m, r[k], b, p))(writer, mask, rec, "a",
    /// buf, pos)`): the caller's string literal for `key`.
    pub(crate) fn helper_member_key(&self, write: &WriteSite) -> Option<String> {
        use oxc_ast::ast::Expression;
        write.helper_call.as_ref()?;
        let value = self.write_value_node(write)?;
        let AstKind::ComputedMemberExpression(member) = self.parse.nodes.kind(value) else {
            return None;
        };
        let formal = |expression: &Expression<'_>| match expression.without_parentheses() {
            Expression::Identifier(identifier) => self.identifier_symbol(identifier).and_then(|symbol| self.formal_index(symbol)),
            _ => None,
        };
        formal(&member.object)?;
        let key = write.helper_arguments.get(formal(&member.expression)?)?;
        let node = self.node_with_span(Span::new(key.start, key.end))?;
        match self.parse.nodes.kind(node) {
            AstKind::StringLiteral(literal) => Some(literal.value.to_string()),
            _ => None,
        }
    }

    /// `t[k]` of the tracer's argument tuple (`t = [v, buf]; w(s, t[0],
    /// t[1], i)`) is the element expression itself, when the nearest
    /// definition of `t` is that array literal and `t` is otherwise only read
    /// as `t[<k>]` (no store through it) or tested. Anything else stays
    /// `node`.
    pub(crate) fn through_tuple(&self, mut node: NodeId) -> NodeId {
        // A tuple element read from another tuple (`d = [c[0], …]`) holds
        // what that element holds.
        for _ in 0..4 {
            let next = self.tuple_step(node);
            if next == node {
                break;
            }
            node = next;
        }
        node
    }

    fn tuple_step(&self, node: NodeId) -> NodeId {
        use oxc_ast::ast::Expression;
        let nodes = self.parse.nodes;
        let scoping = self.parse.scoping;
        let AstKind::ComputedMemberExpression(member) = nodes.kind(node) else {
            return node;
        };
        let Expression::NumericLiteral(index) = member.expression.without_parentheses() else {
            return node;
        };
        let Expression::Identifier(identifier) = member.object.without_parentheses() else {
            return node;
        };
        let Some(symbol) = self.identifier_symbol(identifier) else {
            return node;
        };
        // The nearest definition before this read is the array literal.
        let Some(object) = self.node_with_span(member.object.span()) else {
            return node;
        };
        let reaching = self.slice_of(object, 1);
        let Some(value) = reaching
            .definitions
            .iter()
            .find(|(defined, _, depth)| *defined == symbol && *depth == 1)
            .and_then(|(_, definition, _)| self.definition_value(*definition))
        else {
            return node;
        };
        let AstKind::ArrayExpression(array) = nodes.kind(value) else {
            return node;
        };
        let literal = array.span;
        for reference in scoping.get_resolved_references(symbol) {
            if reference.is_write() {
                continue;
            }
            let read = nodes.kind(reference.node_id()).span();
            // The literal's own reads (`t = [w, t, buf]`) read the old value.
            if literal.start <= read.start && read.end <= literal.end {
                continue;
            }
            let parent = nodes.parent_id(reference.node_id());
            let use_ = match nodes.kind(parent) {
                AstKind::ComputedMemberExpression(use_) => use_,
                // A test (`t == null`, `typeof t`) cannot change it.
                AstKind::BinaryExpression(_) | AstKind::UnaryExpression(_) => continue,
                _ => return node,
            };
            if use_.object.span() != read || !matches!(use_.expression.without_parentheses(), Expression::NumericLiteral(_)) {
                return node;
            }
            let use_span = use_.span;
            let mut ancestor = parent;
            for _ in 0..3 {
                ancestor = nodes.parent_id(ancestor);
                let stored = match nodes.kind(ancestor) {
                    AstKind::AssignmentExpression(assignment) => {
                        let left = assignment.left.span();
                        left.start <= use_span.start && use_span.end <= left.end
                    }
                    AstKind::UpdateExpression(update) => update.argument.span() == use_span,
                    _ => false,
                };
                if stored {
                    return node;
                }
            }
        }
        if index.value.fract() != 0.0 || index.value < 0.0 {
            return node;
        }
        array
            .elements
            .get(index.value as usize)
            .and_then(|element| element.as_expression())
            .and_then(|element| self.node_with_span(element.span()))
            .unwrap_or(node)
    }

    /// The test expression of a `CHOICE` guard span (`if` / conditional test,
    /// logical left operand).
    pub(crate) fn guard_test(&self, guard: Span) -> Option<NodeId> {
        let node = self.node_with_span(guard)?;
        let test = match self.parse.nodes.kind(node) {
            AstKind::IfStatement(statement) => statement.test.span(),
            AstKind::ConditionalExpression(conditional) => conditional.test.span(),
            AstKind::LogicalExpression(logical) => logical.left.span(),
            _ => return None,
        };
        self.node_with_span(test)
    }

    /// The symbol an identifier-reference node resolves to.
    pub(crate) fn reference_symbol(&self, node: NodeId) -> Option<SymbolId> {
        match self.parse.nodes.kind(node) {
            AstKind::IdentifierReference(identifier) => self.identifier_symbol(identifier),
            _ => None,
        }
    }

    pub(crate) fn identifier_symbol(&self, identifier: &IdentifierReference<'_>) -> Option<SymbolId> {
        crate::oxc::identifier_symbol(identifier, self.parse.scoping)
    }

    /// The position of `symbol` among its function's formal parameters.
    fn formal_index(&self, symbol: SymbolId) -> Option<usize> {
        let nodes = self.parse.nodes;
        let declaration = self.parse.scoping.symbol_declaration(symbol);
        let mut node = declaration;
        // The declaration is the binding identifier or its formal parameter.
        for _ in 0..4 {
            if let AstKind::FormalParameter(_) = nodes.kind(node) {
                let parent = nodes.parent_id(node);
                let AstKind::FormalParameters(parameters) = nodes.kind(parent) else {
                    return None;
                };
                let span = nodes.kind(node).span();
                return parameters.items.iter().position(|item| item.span == span);
            }
            let parent = nodes.parent_id(node);
            if parent == node {
                return None;
            }
            node = parent;
        }
        None
    }
}

// ===========================================================================
// Value slices
// ===========================================================================

/// A bounded backward slice of one written value: the value expression and,
/// for every local binding it reads, the nearest definition before the read
/// (a definition in the other arm of a conditional / `if` / logical
/// expression that contains the read is skipped; an assignment whose right
/// side holds the read takes effect after it), up to a hop limit.
///
/// Every fact carries the hop depth that reached it: `0` is the value
/// expression itself, `1` the definition of a binding it reads, and so on.
/// Generated spellings (`$…`) never enter a fact set.
#[derive(Clone, Debug, Default)]
pub(crate) struct ValueSlice {
    /// Static member names (`a.b`, `a["b"]`) with the smallest depth.
    pub members: BTreeMap<String, usize>,
    /// Calls `(method, string arguments, construct)` with the smallest depth.
    pub calls: BTreeMap<(String, Vec<String>, bool), usize>,
    /// Every non-construct call with a named callee: `(method, call node,
    /// depth)`, for binders that read non-string arguments.
    pub call_sites: Vec<(String, NodeId, usize)>,
    /// String literals with the smallest depth.
    pub strings: BTreeMap<String, usize>,
    /// String literals passed directly as a call or `new` argument (any
    /// callee, generated helpers included) with the smallest depth.
    pub arguments: BTreeMap<String, usize>,
    /// Unbound global reads with the smallest depth.
    pub globals: BTreeMap<String, usize>,
    /// Local bindings read, with the depth of the read.
    pub symbols: Vec<(SymbolId, usize)>,
    /// `++x` / `x += …` updates of a followed binding (any position): the
    /// update expression or compound assignment node.
    pub updates: Vec<(SymbolId, NodeId)>,
    /// Definitions followed: `(binding, definition node, depth of its right
    /// side)`.
    pub definitions: Vec<(SymbolId, NodeId, usize)>,
}

impl ValueSlice {
    /// Merge `other` (smallest depth wins).
    pub(crate) fn absorb(&mut self, other: &ValueSlice) {
        for (map, from) in [
            (&mut self.members, &other.members),
            (&mut self.strings, &other.strings),
            (&mut self.arguments, &other.arguments),
            (&mut self.globals, &other.globals),
        ] {
            for (name, depth) in from {
                let entry = map.entry(name.clone()).or_insert(*depth);
                *entry = (*entry).min(*depth);
            }
        }
        for (call, depth) in &other.calls {
            let entry = self.calls.entry(call.clone()).or_insert(*depth);
            *entry = (*entry).min(*depth);
        }
        self.call_sites.extend(other.call_sites.iter().cloned());
        self.symbols.extend(other.symbols.iter().copied());
        self.updates.extend(other.updates.iter().copied());
        self.definitions.extend(other.definitions.iter().copied());
    }

    /// Merge `other`, every depth of it `shift` deeper.
    pub(crate) fn absorb_shifted(&mut self, other: &ValueSlice, shift: usize) {
        let mut shifted = other.clone();
        for map in [&mut shifted.members, &mut shifted.strings, &mut shifted.arguments, &mut shifted.globals] {
            for depth in map.values_mut() {
                *depth += shift;
            }
        }
        for depth in shifted.calls.values_mut() {
            *depth += shift;
        }
        for (_, _, depth) in &mut shifted.call_sites {
            *depth += shift;
        }
        for (_, depth) in &mut shifted.symbols {
            *depth += shift;
        }
        for (_, _, depth) in &mut shifted.definitions {
            *depth += shift;
        }
        self.absorb(&shifted);
    }

    /// Members read at the smallest depth that reads any member.
    pub(crate) fn nearest_members(&self) -> Vec<&str> {
        nearest(&self.members)
    }

    /// Some call of `method` (any depth).
    pub(crate) fn calls_method(&self, method: &str) -> bool {
        self.calls.keys().any(|(name, _, _)| name == method)
    }

    /// `new name(…)` at any depth.
    pub(crate) fn constructs(&self, name: &str) -> bool {
        self.calls.keys().any(|(method, _, construct)| *construct && method == name)
    }

    /// String arguments of calls of `method` (any depth).
    pub(crate) fn string_args(&self, method: &str) -> Vec<&str> {
        self.calls
            .keys()
            .filter(|(name, _, _)| name == method)
            .flat_map(|(_, args, _)| args.iter().map(String::as_str))
            .collect()
    }
}

fn nearest(map: &BTreeMap<String, usize>) -> Vec<&str> {
    let Some(min) = map.values().min() else {
        return Vec::new();
    };
    map.iter()
        .filter(|(_, depth)| *depth == min)
        .map(|(name, _)| name.as_str())
        .collect()
}

#[derive(Hash, PartialEq, Eq)]
struct StartsKey;

/// Every node by `(start, end)`, sorted by start.
struct Starts(Vec<(u32, u32, NodeId)>);

use super::slot_view::generated;

fn keep(map: &mut BTreeMap<String, usize>, name: &str, depth: usize) {
    if generated(name) {
        return;
    }
    let entry = map.entry(name.to_string()).or_insert(depth);
    *entry = (*entry).min(depth);
}

impl FinderCtx<'_, '_, '_> {
    fn starts(&self) -> std::rc::Rc<Starts> {
        self.memo(StartsKey, || {
            let mut starts: Vec<(u32, u32, NodeId)> = self
                .parse
                .nodes
                .iter()
                .map(|node| {
                    let span = node.kind().span();
                    (span.start, span.end, node.id())
                })
                .collect();
            starts.sort_unstable_by_key(|(start, end, id)| (*start, std::cmp::Reverse(*end), id.index()));
            Starts(starts)
        })
    }

    /// Nodes lying inside `span` (including a node with exactly `span`).
    pub(crate) fn nodes_within(&self, span: Span) -> Vec<NodeId> {
        let starts = self.starts();
        let first = starts.0.partition_point(|(start, _, _)| *start < span.start);
        starts.0[first..]
            .iter()
            .take_while(|(start, _, _)| *start < span.end)
            .filter(|(_, end, _)| *end <= span.end)
            .map(|(_, _, id)| *id)
            .collect()
    }

    /// The backward slice of `leaf`'s caller-side value, `hops` definitions
    /// deep.
    pub(crate) fn value_slice(&self, leaf: &LeafView, hops: usize) -> Option<ValueSlice> {
        let value = self.caller_value_node(leaf)?;
        let mut slice = self.slice_of(value, hops);
        // A helper write's value reading a helper formal (`l.length` in
        // `(w, s, l, b, i) => w(s, l.length, b, i)`) reads what the caller
        // passes for it, at the depth of the formal read (the formal is the
        // argument under another name).
        if leaf.write.helper_call.is_some() {
            let formals: Vec<(SymbolId, usize)> = slice.symbols.clone();
            let mut seen = Vec::new();
            for (symbol, depth) in formals {
                if depth > hops || seen.contains(&symbol) {
                    continue;
                }
                seen.push(symbol);
                let Some(argument) = self
                    .formal_index(symbol)
                    .and_then(|index| leaf.write.helper_arguments.get(index))
                    .and_then(|argument| self.node_with_span(Span::new(argument.start, argument.end)))
                else {
                    continue;
                };
                let inner = self.slice_of(argument, hops - depth);
                slice.absorb_shifted(&inner, depth);
            }
        }
        Some(slice)
    }

    /// The backward slice of the expression `node`.
    pub(crate) fn slice_of(&self, node: NodeId, hops: usize) -> ValueSlice {
        let nodes = self.parse.nodes;
        let mut slice = ValueSlice::default();
        let mut seen_defs: Vec<NodeId> = Vec::new();
        let mut seen_symbols: Vec<SymbolId> = Vec::new();
        let mut work: Vec<(Span, u32, usize)> = vec![(nodes.kind(node).span(), nodes.kind(node).span().start, 0)];
        while let Some((span, use_at, depth)) = work.pop() {
            for id in self.nodes_within(span) {
                match nodes.kind(id) {
                    AstKind::StaticMemberExpression(member) => keep(&mut slice.members, &member.property.name, depth),
                    AstKind::ComputedMemberExpression(member) => {
                        if let oxc_ast::ast::Expression::StringLiteral(literal) = &member.expression {
                            keep(&mut slice.members, &literal.value, depth);
                        }
                    }
                    AstKind::StringLiteral(literal) => keep(&mut slice.strings, &literal.value, depth),
                    AstKind::CallExpression(call) => {
                        for argument in string_args(&call.arguments) {
                            keep(&mut slice.arguments, &argument, depth);
                        }
                        if let Some(method) = callee_name(&call.callee) {
                            slice.call_sites.push((method.clone(), id, depth));
                            record_call(&mut slice, method, string_args(&call.arguments), false, depth);
                        }
                    }
                    AstKind::NewExpression(new) => {
                        for argument in string_args(&new.arguments) {
                            keep(&mut slice.arguments, &argument, depth);
                        }
                        if let Some(method) = callee_name(&new.callee) {
                            record_call(&mut slice, method, string_args(&new.arguments), true, depth);
                        }
                    }
                    AstKind::IdentifierReference(identifier) => match self.identifier_symbol(identifier) {
                        None => keep(&mut slice.globals, &identifier.name, depth),
                        Some(symbol) => {
                            slice.symbols.push((symbol, depth));
                            let read_at = identifier.span.start.max(use_at);
                            if depth >= hops {
                                continue;
                            }
                            // `x = x / n` reads the earlier `x`: follow every
                            // read, each definition once.
                            let (mut definition, updates) = self.nearest_definition(symbol, read_at);
                            // A nearest definition followed by a `return`
                            // in a block without the read never reaches it:
                            // the latest one before the read that does.
                            if let Some((found, _)) = definition {
                                if let Some(other) = self.reaching_definition(symbol, found, id) {
                                    let right = match nodes.kind(other) {
                                        AstKind::VariableDeclarator(declarator) => declarator.init.as_ref().map(|init| init.span()),
                                        AstKind::AssignmentExpression(assignment) => Some(assignment.right.span()),
                                        _ => None,
                                    };
                                    if let Some(right) = right {
                                        definition = Some((other, right));
                                    }
                                }
                            }
                            if !seen_symbols.contains(&symbol) {
                                seen_symbols.push(symbol);
                                slice.updates.extend(updates.into_iter().map(|update| (symbol, update)));
                            }
                            if let Some((definition, right)) = definition {
                                if !seen_defs.contains(&definition) {
                                    seen_defs.push(definition);
                                    slice.definitions.push((symbol, definition, depth + 1));
                                    work.push((right, right.start, depth + 1));
                                }
                            }
                        }
                    },
                    _ => {}
                }
            }
        }
        slice
    }

    /// The nearest definition of `symbol` taking effect before offset `at`:
    /// `(definition node, right-hand side span)`, plus every update
    /// (`++x`, `x += …`) of the symbol.
    fn nearest_definition(&self, symbol: SymbolId, at: u32) -> (Option<(NodeId, Span)>, Vec<NodeId>) {
        self.nearest_definition_where(symbol, at, false)
    }

    /// [`Self::nearest_definition`]; with `objects`, definitions assigning
    /// `void 0` / `undefined` / `null` are skipped: a member read through the
    /// binding never sees them (it would throw).
    fn nearest_definition_where(&self, symbol: SymbolId, at: u32, objects: bool) -> (Option<(NodeId, Span)>, Vec<NodeId>) {
        let nodes = self.parse.nodes;
        let scoping = self.parse.scoping;
        let mut definitions: Vec<(u32, NodeId, Span)> = Vec::new();
        let mut updates = Vec::new();
        let declaration = scoping.symbol_declaration(symbol);
        if let AstKind::VariableDeclarator(declarator) = nodes.kind(declaration) {
            if let Some(init) = &declarator.init {
                definitions.push((declarator.span.end, declaration, init.span()));
            }
        }
        for reference in scoping.get_resolved_references(symbol) {
            if !reference.is_write() {
                continue;
            }
            let mut node = reference.node_id();
            for _ in 0..4 {
                let parent = nodes.parent_id(node);
                if parent == node {
                    break;
                }
                node = parent;
                match nodes.kind(node) {
                    AstKind::AssignmentExpression(assignment) => {
                        if assignment.operator.is_assign() {
                            definitions.push((assignment.span.end, node, assignment.right.span()));
                        } else {
                            updates.push(node);
                        }
                        break;
                    }
                    AstKind::UpdateExpression(_) => {
                        updates.push(node);
                        break;
                    }
                    _ => {}
                }
            }
        }
        let empty = |right: Span| {
            use oxc_ast::ast::{Expression, UnaryOperator};
            let Some(id) = self.node_with_span(right) else { return false };
            match nodes.kind(id) {
                AstKind::UnaryExpression(unary) => {
                    unary.operator == UnaryOperator::Void && matches!(unary.argument, Expression::NumericLiteral(_))
                }
                AstKind::NullLiteral(_) => true,
                AstKind::IdentifierReference(identifier) => identifier.name == "undefined",
                _ => false,
            }
        };
        let best = definitions
            .into_iter()
            .filter(|(_, _, right)| !objects || !empty(*right))
            .filter(|(end, node, _)| *end <= at && !self.other_arm(*node, at))
            .max_by_key(|(end, _, _)| *end)
            .map(|(_, node, right)| (node, right));
        (best, updates)
    }

    /// `node` sits in an arm of a conditional, `if` or logical expression
    /// that contains `at` in a different arm (or in its test).
    fn other_arm(&self, node: NodeId, at: u32) -> bool {
        let nodes = self.parse.nodes;
        let contains = |span: Span| span.start <= at && at < span.end;
        let mut child = node;
        loop {
            let parent = nodes.parent_id(child);
            if parent == child {
                return false;
            }
            let child_span = nodes.kind(child).span();
            let parent_kind = nodes.kind(parent);
            if contains(parent_kind.span()) {
                let arm = match parent_kind {
                    AstKind::ConditionalExpression(conditional) => {
                        conditional.consequent.span() == child_span || conditional.alternate.span() == child_span
                    }
                    AstKind::IfStatement(statement) => {
                        statement.consequent.span() == child_span
                            || statement.alternate.as_ref().is_some_and(|alternate| alternate.span() == child_span)
                    }
                    AstKind::LogicalExpression(logical) => logical.right.span() == child_span,
                    _ => false,
                };
                return arm && !contains(child_span);
            }
            child = parent;
        }
    }
}

fn record_call(slice: &mut ValueSlice, method: String, args: Vec<String>, construct: bool, depth: usize) {
    let entry = slice.calls.entry((method, args, construct)).or_insert(depth);
    *entry = (*entry).min(depth);
}

fn callee_name(callee: &oxc_ast::ast::Expression<'_>) -> Option<String> {
    use oxc_ast::ast::Expression;
    match callee.without_parentheses() {
        Expression::Identifier(identifier) if !generated(&identifier.name) => Some(identifier.name.to_string()),
        Expression::StaticMemberExpression(member) if !generated(&member.property.name) => {
            Some(member.property.name.to_string())
        }
        Expression::ComputedMemberExpression(member) => match &member.expression {
            Expression::StringLiteral(literal) => Some(literal.value.to_string()),
            _ => None,
        },
        Expression::ChainExpression(_) => None,
        _ => None,
    }
}

fn string_args(arguments: &[oxc_ast::ast::Argument<'_>]) -> Vec<String> {
    arguments
        .iter()
        .filter_map(|argument| match argument {
            oxc_ast::ast::Argument::StringLiteral(literal) => Some(literal.value.to_string()),
            _ => None,
        })
        .collect()
}

// ===========================================================================
// Record constructions
// ===========================================================================

/// One object built by the program: an object literal's static properties
/// plus the `name.key = value` stores into the binding it initialises.
/// Keys are the collector's own (possibly build-mangled) record keys; they
/// only link a read `record.key` to the value stored under `key`.
#[derive(Clone, Debug)]
pub(crate) struct RecordConstruction {
    /// The object literal.
    pub node: NodeId,
    /// Key -> stored value expression.
    pub values: BTreeMap<String, NodeId>,
}

#[derive(Hash, PartialEq, Eq)]
struct RecordsKey;

struct Records(Vec<RecordConstruction>);

impl FinderCtx<'_, '_, '_> {
    fn records(&self) -> std::rc::Rc<Records> {
        self.memo(RecordsKey, || {
            use oxc_ast::ast::{AssignmentTarget, BindingPattern, Expression, ObjectPropertyKind};
            let nodes = self.parse.nodes;
            // `name.key = value` stores by the binding of `name`.
            let mut stores: HashMap<SymbolId, Vec<(String, NodeId)>> = HashMap::new();
            for node in nodes.iter() {
                let AstKind::AssignmentExpression(assignment) = node.kind() else {
                    continue;
                };
                if !assignment.operator.is_assign() {
                    continue;
                }
                let AssignmentTarget::StaticMemberExpression(member) = &assignment.left else {
                    continue;
                };
                let Expression::Identifier(object) = member.object.without_parentheses() else {
                    continue;
                };
                let (Some(symbol), Some(value)) =
                    (self.identifier_symbol(object), self.node_with_span(assignment.right.span()))
                else {
                    continue;
                };
                if !generated(&member.property.name) {
                    stores.entry(symbol).or_default().push((member.property.name.to_string(), value));
                }
            }
            let mut out = Vec::new();
            for node in nodes.iter() {
                let AstKind::ObjectExpression(object) = node.kind() else {
                    continue;
                };
                let mut values = BTreeMap::new();
                for property in &object.properties {
                    let ObjectPropertyKind::ObjectProperty(property) = property else {
                        continue;
                    };
                    let (Some(key), Some(value)) = (property.key.static_name(), self.node_with_span(property.value.span()))
                    else {
                        continue;
                    };
                    if !generated(&key) {
                        values.insert(key.to_string(), value);
                    }
                }
                // The binding the literal initialises (`let o = {…}`, `o = {…}`).
                let parent = nodes.parent_id(node.id());
                let symbol = match nodes.kind(parent) {
                    AstKind::VariableDeclarator(declarator) if declarator.init.as_ref().is_some_and(|init| init.span() == object.span) => {
                        match &declarator.id {
                            BindingPattern::BindingIdentifier(identifier) => identifier.symbol_id.get(),
                            _ => None,
                        }
                    }
                    AstKind::AssignmentExpression(assignment) if assignment.right.span() == object.span => {
                        match &assignment.left {
                            AssignmentTarget::AssignmentTargetIdentifier(identifier) => self.identifier_symbol(identifier),
                            _ => None,
                        }
                    }
                    _ => None,
                };
                if let Some(symbol) = symbol {
                    // A key the literal only reserves (`{i: void 0}` then
                    // `o.i = v`) stores the assigned value.
                    let placeholder = |node: NodeId| match nodes.kind(node) {
                        AstKind::UnaryExpression(unary) => {
                            unary.operator == oxc_syntax::operator::UnaryOperator::Void
                                && matches!(unary.argument, Expression::NumericLiteral(_))
                        }
                        AstKind::IdentifierReference(identifier) => identifier.name == "undefined",
                        _ => false,
                    };
                    let mut filled = std::collections::BTreeSet::new();
                    for (key, value) in stores.get(&symbol).into_iter().flatten() {
                        match values.get(key) {
                            None => {
                                values.insert(key.clone(), *value);
                            }
                            Some(reserved) if placeholder(*reserved) && filled.insert(key.clone()) => {
                                values.insert(key.clone(), *value);
                            }
                            Some(_) => {}
                        }
                    }
                }
                if !values.is_empty() {
                    out.push(RecordConstruction { node: node.id(), values });
                }
            }
            Records(out)
        })
    }

    /// Every record construction that stores all of `keys`.
    pub(crate) fn records_with_keys(&self, keys: &[&str]) -> Vec<RecordConstruction> {
        self.records()
            .0
            .iter()
            .filter(|record| keys.iter().all(|key| record.values.contains_key(*key)))
            .cloned()
            .collect()
    }

    /// The record construction of the object literal `node`.
    pub(crate) fn record_at(&self, node: NodeId) -> Option<RecordConstruction> {
        self.records().0.iter().find(|record| record.node == node).cloned()
    }

    /// The record key path a value expression reads from its record root:
    /// `item.i` -> `[i]`; `c.k` with `c = item.j` -> `[j, k]`. The root is an
    /// array element (`list[n]`) or a binding without a followed definition.
    pub(crate) fn read_key_path(&self, node: NodeId) -> Option<Vec<String>> {
        use oxc_ast::ast::Expression;
        let nodes = self.parse.nodes;
        let mut path = Vec::new();
        let mut node = node;
        for _ in 0..6 {
            let key = self.read_key(node)?;
            path.push(key);
            let object = match nodes.kind(node) {
                AstKind::StaticMemberExpression(member) => &member.object,
                AstKind::ComputedMemberExpression(member) => &member.object,
                _ => return None,
            };
            match object.without_parentheses() {
                Expression::Identifier(_) => {
                    let base = self.node_with_span(object.without_parentheses().span())?;
                    let slice = self.slice_of(base, 1);
                    match slice.definitions.first().and_then(|(_, definition, _)| self.definition_value(*definition)) {
                        Some(value) if self.read_key(value).is_some() => node = value,
                        _ => break,
                    }
                }
                Expression::ComputedMemberExpression(element)
                    if !matches!(element.expression, Expression::StringLiteral(_)) =>
                {
                    break
                }
                inner => node = self.node_with_span(inner.span())?,
            }
        }
        path.reverse();
        Some(path)
    }

    /// The record key a value expression reads: `record.key` /
    /// `record["key"]` (outer parentheses dropped).
    pub(crate) fn read_key(&self, node: NodeId) -> Option<String> {
        match self.parse.nodes.kind(node) {
            AstKind::StaticMemberExpression(member) if !generated(&member.property.name) => {
                Some(member.property.name.to_string())
            }
            AstKind::ComputedMemberExpression(member) => match &member.expression {
                oxc_ast::ast::Expression::StringLiteral(literal) if !generated(&literal.value) => {
                    Some(literal.value.to_string())
                }
                _ => None,
            },
            AstKind::ParenthesizedExpression(inner) => self.read_key(self.node_with_span(inner.expression.span())?),
            _ => None,
        }
    }
}

impl FinderCtx<'_, '_, '_> {
    /// The right side of a definition node of [`ValueSlice::definitions`]
    /// (declarator init or assignment right side).
    pub(crate) fn definition_value(&self, definition: NodeId) -> Option<NodeId> {
        let span = match self.parse.nodes.kind(definition) {
            AstKind::VariableDeclarator(declarator) => declarator.init.as_ref()?.span(),
            AstKind::AssignmentExpression(assignment) => assignment.right.span(),
            _ => return None,
        };
        self.node_with_span(span)
    }
}

// ===========================================================================
// Guard strings and member paths
// ===========================================================================

/// One step of a [`FinderCtx::member_path`].
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum PathStep {
    /// `.name` / `["name"]`.
    Property(String),
    /// `[n]` with a number literal.
    Index(f64),
    /// `[expression]`.
    Dynamic,
}

impl FinderCtx<'_, '_, '_> {
    /// The string literals of every `CHOICE` guard test enclosing `write`,
    /// outermost first (an `in` / `hasOwn(obj, "key")` guard names its key).
    pub(crate) fn guard_strings(&self, write: &WriteSite) -> Vec<Vec<String>> {
        write
            .guards
            .iter()
            .map(|guard| {
                let Some(test) = self.guard_test(*guard) else { return Vec::new() };
                self.nodes_within(self.parse.nodes.kind(test).span())
                    .into_iter()
                    .filter_map(|id| match self.parse.nodes.kind(id) {
                        AstKind::StringLiteral(literal) => Some(literal.value.to_string()),
                        _ => None,
                    })
                    .collect()
            })
            .collect()
    }

    /// The member-access chain an expression reads, from its base outward:
    /// `r.b.a[i][2]` with `r = list[k]` is `[Dynamic, b, a, Dynamic, 2]`.
    /// A local binding is replaced by its nearest definition before the read
    /// (as in [`Self::slice_of`], so `x = x.b` reads the earlier `x`), up to
    /// `hops` definitions; the chain stops at any other expression. Under a
    /// pending member step the definition skips `void 0` / `null` writes, and
    /// `pair[1]` of a pair literal (`pair = [f, obj]`) reads `obj`.
    pub(crate) fn member_path(&self, node: NodeId, hops: usize) -> Vec<PathStep> {
        use oxc_ast::ast::Expression;
        let nodes = self.parse.nodes;
        let mut steps = Vec::new();
        let mut current = node;
        let mut use_at = nodes.kind(node).span().start;
        let mut left = hops;
        loop {
            let kind = nodes.kind(current);
            let next = match kind {
                AstKind::ParenthesizedExpression(inner) => Some(inner.expression.span()),
                AstKind::StaticMemberExpression(member) => {
                    steps.push(PathStep::Property(member.property.name.to_string()));
                    Some(member.object.span())
                }
                AstKind::ComputedMemberExpression(member) => {
                    steps.push(match &member.expression {
                        Expression::StringLiteral(literal) => PathStep::Property(literal.value.to_string()),
                        Expression::NumericLiteral(literal) => PathStep::Index(literal.value),
                        _ => PathStep::Dynamic,
                    });
                    Some(member.object.span())
                }
                AstKind::IdentifierReference(identifier) => {
                    let Some(symbol) = self.identifier_symbol(identifier) else { break };
                    if left == 0 {
                        break;
                    }
                    left -= 1;
                    let read_at = identifier.span.start.max(use_at);
                    // A member read never reads an emptied register.
                    let (definition, _) = self.nearest_definition_where(symbol, read_at, !steps.is_empty());
                    let Some((_, right)) = definition else { break };
                    use_at = right.start;
                    // `pair = [write, obj]; pair[1].p`: the indexed element
                    // of the pair literal.
                    let element = match (steps.last(), self.node_with_span(right).map(|id| nodes.kind(id))) {
                        (Some(PathStep::Index(index)), Some(AstKind::ArrayExpression(array))) if index.fract() == 0.0 => array
                            .elements
                            .get(*index as usize)
                            .and_then(|element| element.as_expression())
                            .map(|element| element.span()),
                        _ => None,
                    };
                    match element {
                        Some(span) => {
                            steps.pop();
                            Some(span)
                        }
                        None => Some(right),
                    }
                }
                _ => None,
            };
            let Some(span) = next else { break };
            let Some(id) = self.node_with_span(span) else { break };
            current = id;
        }
        steps.reverse();
        steps
    }
}

// ===========================================================================
// Producer callback ABI
// ===========================================================================

/// One call of a producer formal that passes a callback: `(formal position,
/// callback function span, call span)`.
pub(crate) type RegistrarCall = (usize, crate::signals::schema::SourceSpan, crate::signals::schema::SourceSpan);

impl FinderCtx<'_, '_, '_> {
    /// Calls of the formals of the producer function `producer` (its exact
    /// span) that pass a function or arrow literal, for producers whose
    /// lifecycle registrations were not proven (an unrecovered coordinator).
    /// `None` unless formal 0 is read as `p[k]` (the emitter array of the
    /// producer callback ABI `(emitters, context, init, collection,
    /// last_chance, cleanup)`).
    pub(crate) fn registrar_calls(&self, producer: crate::signals::schema::SourceSpan) -> Option<Vec<RegistrarCall>> {
        use oxc_ast::ast::{Argument, Expression};
        let nodes = self.parse.nodes;
        let scoping = self.parse.scoping;
        let exact = Span::new(producer.start, producer.end);
        let mut function = self.node_with_span(exact)?;
        let params = loop {
            match nodes.kind(function) {
                AstKind::Function(function) => break &function.params,
                AstKind::ArrowFunctionExpression(arrow) => break &arrow.params,
                _ => {}
            }
            let parent = nodes.parent_id(function);
            if parent == function || nodes.kind(parent).span() != exact {
                return None;
            }
            function = parent;
        };
        let formals: Vec<Option<SymbolId>> = params
            .items
            .iter()
            .map(|param| param.pattern.get_binding_identifier().and_then(|binding| binding.symbol_id.get()))
            .collect();
        let emitters_indexed = formals.first().copied().flatten().is_some_and(|symbol| {
            scoping.get_resolved_references(symbol).any(|reference| {
                matches!(nodes.kind(nodes.parent_id(reference.node_id())), AstKind::ComputedMemberExpression(_))
            })
        });
        if !emitters_indexed {
            return None;
        }
        let mut out = Vec::new();
        for (position, symbol) in formals.iter().enumerate().skip(1) {
            let Some(symbol) = symbol else {
                continue;
            };
            for reference in scoping.get_resolved_references(*symbol) {
                let AstKind::CallExpression(call) = nodes.kind(nodes.parent_id(reference.node_id())) else {
                    continue;
                };
                let reference_span = nodes.get_node(reference.node_id()).span();
                if !matches!(call.callee.without_parentheses(), Expression::Identifier(id) if id.span == reference_span) {
                    continue;
                }
                for argument in call.arguments.iter().filter_map(Argument::as_expression) {
                    let span = match argument.without_parentheses() {
                        Expression::FunctionExpression(function) => function.span,
                        Expression::ArrowFunctionExpression(arrow) => arrow.span,
                        _ => continue,
                    };
                    out.push((
                        position,
                        crate::signals::schema::SourceSpan { start: span.start, end: span.end },
                        crate::signals::schema::SourceSpan { start: call.span.start, end: call.span.end },
                    ));
                }
            }
        }
        Some(out)
    }
}
