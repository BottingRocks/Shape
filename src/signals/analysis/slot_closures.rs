//! Slot-local host evidence: the members and calls reached from one slot's
//! own sites, each tagged with the fewest def-use hops that reached it.
//!
//! A group producer serves up to five signals whose drawing code is
//! interleaved, so [`SlotView::host`] cannot tell a canvas slot from an audio
//! slot of the same row. Two closures are anchored on the slot's own sites
//! ([`SlotView::anchors`]):
//!
//! * **anchor closure** — the statements holding the slot's anchors (its
//!   writes, emission sites, value sources), then, up to [`HOPS`] times,
//!   every statement that mentions a binding mentioned by a statement
//!   already reached (bindings with many references, i.e. shared helpers,
//!   are not followed). A reached statement also contributes its inline
//!   callbacks and the helper functions it calls (through `let a = b` /
//!   `a = b` aliases). The innermost function around an anchor, when it is
//!   not the producer or a registered callback itself (an `onload` handler,
//!   a subscriber, a glyph-sampling helper), counts at depth 1, and so do
//!   the statements that hand it on by name;
//! * **value closure** — the same walk seeded only by the value roots: the
//!   arrays/objects read by the slot's writer calls (`X` of
//!   `write(stream, X[i], buffer)` or `X.length`), so a shared buffer,
//!   stream or loop counter does not pull in a neighbour's code.
//!
//! Plus the backward slices of the repeated values ([`value_slices`]) and of
//! the `if` tests around the writes ([`guard_slices`]). Only static member
//! names and called method names (with their string arguments) are
//! collected; generated `$` spellings are skipped.
//!
//! Used by: no signal directly; helper modules
//! [`render_probes`](super::render_probes).

use super::slot_view::generated;

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::rc::Rc;

use oxc_ast::ast::{Argument, Expression};
use oxc_ast::AstKind;

use crate::signals::analysis::{FinderCtx, SlotView};

/// Def-use hops from the anchor statements.
pub(crate) const HOPS: usize = 4;
/// Bindings referenced more often than this are shared plumbing.
const MAX_REFERENCES: usize = 24;
/// Own functions larger than this are shared infrastructure.
const MAX_FUNCTION_BYTES: u32 = 40_000;

/// Slot-local facts, each with the fewest def-use hops that reached it
/// (own-function facts count as [`OWN_FUNCTION_DEPTH`]).
#[derive(Clone, Debug, Default)]
pub(crate) struct LocalFacts {
    pub members: BTreeMap<String, usize>,
    /// `(method, string args, construct) -> depth`.
    pub calls: BTreeMap<(String, Vec<String>, bool), usize>,
}

/// The depth assigned to facts of an own function.
pub(crate) const OWN_FUNCTION_DEPTH: usize = 1;

impl LocalFacts {
    /// Smallest depth of a member read or call of `name`.
    pub(crate) fn depth(&self, name: &str) -> Option<usize> {
        let member = self.members.get(name).copied();
        let call = self
            .calls
            .iter()
            .filter(|((method, _, _), _)| method == name)
            .map(|(_, depth)| *depth)
            .min();
        match (member, call) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    fn absorb(&mut self, facts: &StmtFacts, depth: usize) {
        for member in &facts.members {
            let entry = self.members.entry(member.clone()).or_insert(depth);
            *entry = (*entry).min(depth);
        }
        for call in &facts.calls {
            let entry = self.calls.entry(call.clone()).or_insert(depth);
            *entry = (*entry).min(depth);
        }
    }
}

#[derive(Clone, Debug, Default)]
struct StmtFacts {
    members: Vec<String>,
    calls: Vec<(String, Vec<String>, bool)>,
    symbols: Vec<u32>,
    /// Inline callbacks (function expressions) of this statement.
    nested: Vec<(u32, u32)>,
    /// Bindings called as `f(...)`.
    callees: Vec<u32>,
}

struct Index {
    /// Statement spans `(start, end, statement)`, sorted by start.
    statements: Vec<(u32, u32, u32)>,
    /// Own facts of each statement (nested statements excluded).
    facts: HashMap<u32, StmtFacts>,
    /// Statements mentioning each binding.
    symbol_statements: HashMap<u32, Vec<u32>>,
    /// Function spans, sorted by start.
    functions: Vec<(u32, u32)>,
    /// The function a binding is declared as (`function f`, `let f = () => …`).
    symbol_function: HashMap<u32, (u32, u32)>,
    /// `let a = b` aliases.
    symbol_alias: HashMap<u32, u32>,
    /// The bindings naming each function span.
    function_symbols: HashMap<(u32, u32), Vec<u32>>,
    /// Roots of the member-expression arguments of each call, by call start.
    call_roots: HashMap<u32, Vec<u32>>,
    /// Starts of every `x.next([…])` call, sorted.
    emissions: Vec<u32>,
    /// Bindings stepped by `++`/`--` (loop counters, reused across
    /// unrelated loops): never followed.
    counters: std::collections::HashSet<u32>,
    memo: RefCell<HashMap<Vec<u32>, Rc<Local>>>,
}

/// Inline callbacks and called helpers larger than this are not absorbed.
const MAX_CALLBACK_BYTES: u32 = 12_000;
/// A value root mentioned in more statements than this is not a root.
const MAX_ROOT_REFERENCES: usize = 60;

/// Memo key of the program index (per analysis, in [`FinderCtx::memo`]).
#[derive(Hash, PartialEq, Eq)]
struct IndexKey;


fn callee_name(callee: &Expression<'_>) -> Option<String> {
    match callee.without_parentheses() {
        Expression::Identifier(identifier) if !generated(&identifier.name) => {
            Some(identifier.name.to_string())
        }
        Expression::StaticMemberExpression(member) if !generated(&member.property.name) => {
            Some(member.property.name.to_string())
        }
        Expression::ComputedMemberExpression(member) => match &member.expression {
            Expression::StringLiteral(literal) => Some(literal.value.to_string()),
            _ => None,
        },
        _ => None,
    }
}

fn string_args(arguments: &[Argument<'_>]) -> Vec<String> {
    arguments
        .iter()
        .filter_map(|argument| match argument {
            Argument::StringLiteral(literal) => Some(literal.value.to_string()),
            _ => None,
        })
        .collect()
}

fn build(ctx: &FinderCtx) -> Index {
    const NONE: u32 = u32::MAX;
    let nodes = ctx.parse.nodes;
    let scoping = ctx.parse.scoping;
    let mut statement_of = vec![NONE; nodes.len()];
    let mut statements = Vec::new();
    let mut facts: HashMap<u32, StmtFacts> = HashMap::new();
    let mut functions = Vec::new();
    let mut symbol_function = HashMap::new();
    let mut symbol_alias = HashMap::new();
    let mut call_roots: HashMap<u32, Vec<u32>> = HashMap::new();
    let mut emissions: Vec<u32> = Vec::new();
    let mut counters: std::collections::HashSet<u32> = std::collections::HashSet::new();
    for (id, node) in nodes.iter_enumerated() {
        let at = id.index();
        let parent = nodes.parent_id(id);
        let kind = node.kind();
        let inherited = if parent == id || parent.index() >= at {
            NONE
        } else {
            statement_of[parent.index()]
        };
        let own = if kind.is_statement() {
            let span = oxc_span::GetSpan::span(&kind);
            statements.push((span.start, span.end, at as u32));
            at as u32
        } else {
            inherited
        };
        statement_of[at] = own;
        let function_span = match kind {
            AstKind::Function(function) => {
                if let Some(symbol) = function.id.as_ref().and_then(|id| id.symbol_id.get()) {
                    symbol_function.insert(symbol.index() as u32, (function.span.start, function.span.end));
                }
                Some((function.span.start, function.span.end, function.is_expression()))
            }
            AstKind::ArrowFunctionExpression(arrow) => Some((arrow.span.start, arrow.span.end, true)),
            _ => None,
        };
        if let Some((start, end, _)) = function_span {
            functions.push((start, end));
        }
        if let AstKind::VariableDeclarator(declarator) = kind {
            let symbol = match &declarator.id {
                oxc_ast::ast::BindingPattern::BindingIdentifier(identifier) => identifier.symbol_id.get(),
                _ => None,
            };
            if let (Some(symbol), Some(init)) = (symbol, &declarator.init) {
                match init.without_parentheses() {
                    Expression::FunctionExpression(function) => {
                        symbol_function.insert(symbol.index() as u32, (function.span.start, function.span.end));
                    }
                    Expression::ArrowFunctionExpression(arrow) => {
                        symbol_function.insert(symbol.index() as u32, (arrow.span.start, arrow.span.end));
                    }
                    Expression::Identifier(identifier) => {
                        if let Some(target) = identifier
                            .reference_id
                            .get()
                            .and_then(|reference| scoping.get_reference(reference).symbol_id())
                        {
                            symbol_alias.insert(symbol.index() as u32, target.index() as u32);
                        }
                    }
                    _ => {}
                }
            }
        }
        if let AstKind::UpdateExpression(update) = kind {
            if let oxc_ast::ast::SimpleAssignmentTarget::AssignmentTargetIdentifier(target) = &update.argument {
                if let Some(symbol) = target
                    .reference_id
                    .get()
                    .and_then(|reference| scoping.get_reference(reference).symbol_id())
                {
                    counters.insert(symbol.index() as u32);
                }
            }
        }
        if let AstKind::AssignmentExpression(assignment) = kind {
            if let (
                oxc_ast::ast::AssignmentTarget::AssignmentTargetIdentifier(target),
                Expression::Identifier(value),
            ) = (&assignment.left, assignment.right.without_parentheses())
            {
                let resolve = |reference: Option<oxc_syntax::reference::ReferenceId>| {
                    reference.and_then(|reference| scoping.get_reference(reference).symbol_id())
                };
                if let (Some(target), Some(value)) =
                    (resolve(target.reference_id.get()), resolve(value.reference_id.get()))
                {
                    symbol_alias.entry(target.index() as u32).or_insert(value.index() as u32);
                }
            }
        }
        if own == NONE {
            continue;
        }
        let entry = || StmtFacts::default();
        if let Some((start, end, true)) = function_span {
            facts.entry(own).or_insert_with(entry).nested.push((start, end));
        }
        if let AstKind::CallExpression(call) = kind {
            // `observer.next([[], buffer])`: an emission site.
            if let Expression::StaticMemberExpression(member) = call.callee.without_parentheses() {
                let payload = call.arguments.first().and_then(|argument| argument.as_expression());
                if member.property.name == "next"
                    && matches!(payload.map(|e| e.without_parentheses()), Some(Expression::ArrayExpression(_)))
                {
                    emissions.push(call.span.start);
                }
            }
            let mut roots = Vec::new();
            for argument in &call.arguments {
                let Some(mut expression) = argument.as_expression() else { continue };
                let mut through_member = false;
                loop {
                    match expression.without_parentheses() {
                        Expression::ComputedMemberExpression(member) => {
                            expression = &member.object;
                            through_member = true;
                        }
                        Expression::StaticMemberExpression(member) => {
                            expression = &member.object;
                            through_member = true;
                        }
                        _ => break,
                    }
                }
                if !through_member {
                    continue;
                }
                if let Expression::Identifier(identifier) = expression.without_parentheses() {
                    if let Some(symbol) = identifier
                        .reference_id
                        .get()
                        .and_then(|reference| scoping.get_reference(reference).symbol_id())
                    {
                        roots.push(symbol.index() as u32);
                    }
                }
            }
            if !roots.is_empty() {
                call_roots.entry(call.span.start).or_insert_with(Vec::new).extend(roots);
            }
            if let Expression::Identifier(identifier) = call.callee.without_parentheses() {
                if let Some(symbol) = identifier
                    .reference_id
                    .get()
                    .and_then(|reference| scoping.get_reference(reference).symbol_id())
                {
                    facts.entry(own).or_insert_with(entry).callees.push(symbol.index() as u32);
                }
            }
        }
        match kind {
            AstKind::StaticMemberExpression(member) if !generated(&member.property.name) => {
                facts.entry(own).or_insert_with(entry).members.push(member.property.name.to_string());
            }
            AstKind::ComputedMemberExpression(member) => {
                if let Expression::StringLiteral(literal) = &member.expression {
                    facts.entry(own).or_insert_with(entry).members.push(literal.value.to_string());
                }
            }
            AstKind::CallExpression(call) => {
                if let Some(method) = callee_name(&call.callee) {
                    facts
                        .entry(own)
                        .or_insert_with(entry)
                        .calls
                        .push((method, string_args(&call.arguments), false));
                }
            }
            AstKind::NewExpression(new) => {
                if let Some(method) = callee_name(&new.callee) {
                    facts
                        .entry(own)
                        .or_insert_with(entry)
                        .calls
                        .push((method, string_args(&new.arguments), true));
                }
            }
            AstKind::IdentifierReference(identifier) => {
                let symbol = identifier
                    .reference_id
                    .get()
                    .and_then(|reference| scoping.get_reference(reference).symbol_id());
                if let Some(symbol) = symbol {
                    facts.entry(own).or_insert_with(entry).symbols.push(symbol.index() as u32);
                }
            }
            AstKind::BindingIdentifier(identifier) => {
                if let Some(symbol) = identifier.symbol_id.get() {
                    facts.entry(own).or_insert_with(entry).symbols.push(symbol.index() as u32);
                }
            }
            _ => {}
        }
    }
    statements.sort_unstable();
    functions.sort_unstable();
    let mut symbol_statements: HashMap<u32, Vec<u32>> = HashMap::new();
    for (statement, own) in &mut facts {
        own.symbols.sort_unstable();
        own.symbols.dedup();
        for symbol in &own.symbols {
            symbol_statements.entry(*symbol).or_default().push(*statement);
        }
    }
    let mut function_symbols: HashMap<(u32, u32), Vec<u32>> = HashMap::new();
    for (symbol, span) in &symbol_function {
        function_symbols.entry(*span).or_default().push(*symbol);
    }
    Index {
        statements,
        facts,
        symbol_statements,
        functions,
        symbol_function,
        function_symbols,
        symbol_alias,
        call_roots,
        emissions: {
            emissions.sort_unstable();
            emissions
        },
        counters,
        memo: RefCell::new(HashMap::new()),
    }
}

impl Index {
    /// The function a called binding resolves to, through up to four
    /// `let a = b` aliases.
    fn callee_function(&self, mut symbol: u32) -> Option<(u32, u32)> {
        for _ in 0..5 {
            if let Some(span) = self.symbol_function.get(&symbol) {
                return Some(*span);
            }
            symbol = *self.symbol_alias.get(&symbol)?;
        }
        None
    }

    /// Absorb every statement inside `start..end` at `depth`.
    fn absorb_function(&self, out: &mut LocalFacts, start: u32, end: u32, depth: usize) {
        let first = self.statements.partition_point(|(s, _, _)| *s < start);
        for (s, _, statement) in &self.statements[first..] {
            if *s >= end {
                break;
            }
            if let Some(own) = self.facts.get(statement) {
                out.absorb(own, depth);
            }
        }
    }
}

fn index(ctx: &FinderCtx) -> Rc<Index> {
    ctx.memo(IndexKey, || build(ctx))
}

/// The innermost interval of a start-sorted, properly nested list that
/// contains `offset`.
fn innermost(intervals: &[(u32, u32, u32)], offset: u32) -> Option<(u32, u32, u32)> {
    let first_after = intervals.partition_point(|(start, _, _)| *start <= offset);
    intervals[..first_after]
        .iter()
        .rev()
        .find(|(_, end, _)| *end > offset)
        .copied()
}

/// Breadth-first def-use closure from `seeds` (statement, depth).
/// Statements in `stop` are absorbed but not followed.
fn closure(index: &Index, seeds: Vec<(u32, usize)>, stop: &BTreeSet<u32>, out: &mut LocalFacts) {
    let mut seen: BTreeMap<u32, usize> = BTreeMap::new();
    let mut queue = VecDeque::new();
    for (statement, depth) in seeds {
        if !seen.contains_key(&statement) {
            seen.insert(statement, depth);
            queue.push_back((statement, depth));
        }
    }
    let mut followed = BTreeSet::new();
    while let Some((statement, depth)) = queue.pop_front() {
        let Some(own) = index.facts.get(&statement) else { continue };
        out.absorb(own, depth);
        if depth >= HOPS || stop.contains(&statement) {
            continue;
        }
        for (start, end) in &own.nested {
            if end - start <= MAX_CALLBACK_BYTES {
                index.absorb_function(out, *start, *end, depth + 1);
            }
        }
        for callee in &own.callees {
            if let Some((start, end)) = index.callee_function(*callee) {
                if end - start <= MAX_CALLBACK_BYTES {
                    index.absorb_function(out, start, end, depth + 1);
                }
            }
        }
        for symbol in &own.symbols {
            if !followed.insert(*symbol) || index.counters.contains(symbol) {
                continue;
            }
            let Some(statements) = index.symbol_statements.get(symbol) else { continue };
            if statements.len() > MAX_REFERENCES {
                continue;
            }
            for next in statements {
                if !seen.contains_key(next) {
                    seen.insert(*next, depth + 1);
                    queue.push_back((*next, depth + 1));
                }
            }
        }
    }
}

/// Both slot-local views.
#[derive(Clone, Debug, Default)]
pub(crate) struct Local {
    /// Closure from every slot anchor, plus own functions.
    pub anchor: LocalFacts,
    /// Closure from the value roots only: the arrays/objects read by the
    /// slot's writer calls (`X` of `write(s, X[i], buf)` / `X.length`).
    /// Buffers, streams and loop counters are not roots.
    pub value: LocalFacts,
}

/// The anchor and value closures of `view` (memoized per analysis and anchor set).
pub(crate) fn facts(view: &SlotView, ctx: &FinderCtx) -> Rc<Local> {
    let index = index(ctx);
    let anchors: Vec<u32> = view.anchors().to_vec();
    if let Some(found) = index.memo.borrow().get(&anchors) {
        return found.clone();
    }
    let mut out = Local::default();
    // Own functions.
    let spans = view.spans();
    let functions: Vec<(u32, u32, u32)> = index.functions.iter().map(|(s, e)| (*s, *e, 0)).collect();
    let mut own_functions = BTreeSet::new();
    for anchor in &anchors {
        let Some((start, end, _)) = innermost(&functions, *anchor) else { continue };
        let registered = spans.iter().any(|span| {
            let nested = (span.start <= start && end <= span.end) || (start <= span.start && span.end <= end);
            let (short, long) = {
                let a = end - start;
                let b = span.end - span.start;
                (a.min(b), a.max(b))
            };
            nested && u64::from(short) * 10 >= u64::from(long) * 9
        });
        if registered || end - start > MAX_FUNCTION_BYTES {
            continue;
        }
        own_functions.insert((start, end));
    }
    // Anchor closure: the anchor statements, plus the statements that
    // hand an own function on by name (`{ next: f }`, `.then(f)`).
    let mut seeds: Vec<(u32, usize)> = anchors
        .iter()
        .filter_map(|anchor| innermost(&index.statements, *anchor).map(|(_, _, statement)| (statement, 0)))
        .collect();
    for span in &own_functions {
        for symbol in index.function_symbols.get(span).into_iter().flatten() {
            for statement in index.symbol_statements.get(symbol).into_iter().flatten() {
                seeds.push((*statement, OWN_FUNCTION_DEPTH));
            }
        }
    }
    seeds.sort_by_key(|(_, depth)| *depth);
    closure(&index, seeds, &BTreeSet::new(), &mut out.anchor);
    for (start, end) in own_functions {
        index.absorb_function(&mut out.anchor, start, end, OWN_FUNCTION_DEPTH);
        // A named own function (`function onNext(list) {…}` handed to a
        // subscription) sits in the block that builds what it receives;
        // a small enough block counts one hop further.
        if index.function_symbols.contains_key(&(start, end)) {
            if let Some((block_start, block_end, _)) = innermost(&index.statements, start) {
                if block_end - block_start <= MAX_CALLBACK_BYTES {
                    index.absorb_function(&mut out.anchor, block_start, block_end, OWN_FUNCTION_DEPTH + 1);
                }
            }
        }
    }
    // Value roots: from the slot's own writer calls, i.e. those whose next
    // emission site in source order is one of this slot's (a reused
    // buffer or temporary can make a neighbour's writes look like ours).
    let own_emissions: BTreeSet<u32> = anchors
        .iter()
        .copied()
        .filter(|anchor| index.emissions.binary_search(anchor).is_ok())
        .collect();
    let own_writer = |anchor: u32| {
        if own_emissions.is_empty() {
            return true;
        }
        let next = index.emissions.partition_point(|start| *start < anchor);
        index.emissions.get(next).is_some_and(|start| own_emissions.contains(start))
    };
    let mut roots = BTreeSet::new();
    // The writer statements name the buffer and stream too; they are
    // read but not followed.
    let mut writers = BTreeSet::new();
    for anchor in &anchors {
        if let Some(found) = index.call_roots.get(anchor) {
            if own_writer(*anchor) {
                roots.extend(found.iter().copied());
            }
            if let Some((_, _, statement)) = innermost(&index.statements, *anchor) {
                writers.insert(statement);
            }
        }
    }
    let mut seeds = Vec::new();
    for root in &roots {
        if let Some(statements) = index.symbol_statements.get(root) {
            if statements.len() <= MAX_ROOT_REFERENCES {
                seeds.extend(statements.iter().map(|statement| (*statement, 0)));
            }
        }
    }
    closure(&index, seeds, &writers, &mut out.value);
    let out = Rc::new(out);
    index.memo.borrow_mut().insert(anchors, out.clone());
    out
}

/// The fewest def-use hops at which `facts` touch every API of `apis`.
pub(crate) fn api_depth(facts: &LocalFacts, apis: &[&str]) -> Option<usize> {
    apis.iter()
        .map(|api| facts.depth(api))
        .collect::<Option<Vec<usize>>>()
        .and_then(|depths| depths.into_iter().max())
}

/// The fewest def-use hops at which `facts` touch any API of `apis`.
pub(crate) fn any_api_depth(facts: &LocalFacts, apis: &[&str]) -> Option<usize> {
    apis.iter().filter_map(|api| facts.depth(api)).min()
}

/// Deepest hop count that still counts as slot-local evidence.
pub(crate) const MAX_EVIDENCE_DEPTH: usize = 3;

/// Facts of the backward slices of the slot's repeated values (each
/// definition followed, [`MAX_EVIDENCE_DEPTH`] deep): a value array whose
/// binding is reused for the drawing context, the pixel data and the sample
/// list (`x = x.getImageData(…).data; x = [slice(x, …), …]`) is shared
/// plumbing to the closures, but its slice follows the reassignments. A
/// list filled element by element (`list.push(v)`, `list[k] = v`) adds the
/// slices of the stored values, one hop further.
pub(crate) fn value_slices(view: &SlotView, ctx: &FinderCtx) -> LocalFacts {
    value_slices_within(view, ctx, MAX_EVIDENCE_DEPTH)
}

/// Hops a `fed` candidate's API may sit from the written values: the fresh
/// traces route a probe result through copies before it is stored
/// (`r.push(v.canPlayType(t)); x = r[j]; c = x === "" ? 0 : …; out[j] = c`).
pub(crate) const FED_DEPTH: usize = MAX_EVIDENCE_DEPTH + 2;

/// [`value_slices`] followed `max_depth` hops.
pub(crate) fn value_slices_within(view: &SlotView, ctx: &FinderCtx, max_depth: usize) -> LocalFacts {
    use oxc_ast::ast::{AssignmentTarget, Expression};
    use oxc_ast::AstKind;
    use oxc_span::GetSpan;
    let nodes = ctx.parse.nodes;
    let mut facts = LocalFacts::default();
    let mut work: Vec<(crate::signals::analysis::write_sites::ValueSlice, usize)> = view
        .leaves()
        .filter(|leaf| leaf.repeat_depth > 0)
        .filter_map(|leaf| ctx.value_slice(leaf, max_depth))
        .map(|slice| (slice, 0))
        .collect();
    let mut filled = std::collections::BTreeSet::new();
    while let Some((slice, offset)) = work.pop() {
        for (member, depth) in &slice.members {
            let entry = facts.members.entry(member.clone()).or_insert(depth + offset);
            *entry = (*entry).min(depth + offset);
        }
        for (call, depth) in &slice.calls {
            let entry = facts.calls.entry(call.clone()).or_insert(depth + offset);
            *entry = (*entry).min(depth + offset);
        }
        // A list filled element by element (`list.push(v)`, `list[k] = v`):
        // the stored values are one more hop.
        for (symbol, depth) in &slice.symbols {
            let reached = depth + offset;
            if reached + 1 >= max_depth || !filled.insert(*symbol) {
                continue;
            }
            for reference in ctx.parse.scoping.get_resolved_references(*symbol) {
                let member = nodes.parent_id(reference.node_id());
                let stored: Vec<oxc_span::Span> = match nodes.kind(member) {
                    AstKind::StaticMemberExpression(push) if push.property.name == "push" => {
                        match nodes.kind(nodes.parent_id(member)) {
                            AstKind::CallExpression(call)
                                if matches!(call.callee.without_parentheses(), Expression::StaticMemberExpression(callee) if callee.span == push.span) =>
                            {
                                call.arguments.iter().filter_map(|argument| argument.as_expression().map(|e| e.span())).collect()
                            }
                            // The push method taken out of the list (`p =
                            // list.push; p.call(list, v)` or the tracer's
                            // `t = [list, list.push]; t[1].call(t[0], v)`).
                            _ => extracted_push_values(ctx, member),
                        }
                    }
                    AstKind::ComputedMemberExpression(element) => {
                        let mut assignment = nodes.parent_id(member);
                        if !matches!(nodes.kind(assignment), AstKind::AssignmentExpression(_)) {
                            assignment = nodes.parent_id(assignment);
                        }
                        match nodes.kind(assignment) {
                            AstKind::AssignmentExpression(assign)
                                if assign.operator.is_assign()
                                    && matches!(&assign.left, AssignmentTarget::ComputedMemberExpression(target) if target.span == element.span) =>
                            {
                                let mut stored = vec![assign.right.span()];
                                stored.extend(choosing_tests(ctx, assignment, &assign.right));
                                stored
                            }
                            _ => Vec::new(),
                        }
                    }
                    // Held in a register tuple whose element stores fill it
                    // (`t = [k, list]; t[1][t[0]] = v`).
                    AstKind::ArrayExpression(_) => tuple_element_stores(ctx, reference.node_id()),
                    _ => Vec::new(),
                };
                for span in stored {
                    let Some(node) = ctx.node_with_span(span) else { continue };
                    work.push((ctx.slice_of(node, max_depth - reached - 1), reached + 1));
                }
            }
        }
    }
    facts
}

/// The `if` tests that choose a stored code inside a loop: `if (r === "")
/// { c = 0; } else if (r === "maybe") { c = 1; } … list[k] = c` (the
/// tracer's form of `c = r === "" ? 0 : …`). Every write of `c` before the
/// store in the same loop body is under those tests; their spans count as
/// the stored value's.
fn choosing_tests(ctx: &FinderCtx, store: oxc_semantic::NodeId, value: &oxc_ast::ast::Expression<'_>) -> Vec<oxc_span::Span> {
    use oxc_ast::ast::Expression;
    use oxc_ast::AstKind;
    use oxc_span::GetSpan;
    let nodes = ctx.parse.nodes;
    let scoping = ctx.parse.scoping;
    let Expression::Identifier(identifier) = value.without_parentheses() else {
        return Vec::new();
    };
    let Some(symbol) = crate::analysis::identifier_symbol(identifier, scoping) else {
        return Vec::new();
    };
    let is_loop = |id: oxc_semantic::NodeId| {
        matches!(
            nodes.kind(id),
            AstKind::WhileStatement(_) | AstKind::DoWhileStatement(_) | AstKind::ForStatement(_) | AstKind::ForInStatement(_) | AstKind::ForOfStatement(_)
        )
    };
    let is_function = |id: oxc_semantic::NodeId| matches!(nodes.kind(id), AstKind::Function(_) | AstKind::ArrowFunctionExpression(_));
    let Some(body) = nodes.ancestor_ids(store).take_while(|id| !is_function(*id)).find(|id| is_loop(*id)) else {
        return Vec::new();
    };
    let (start, end) = (nodes.kind(body).span().start, nodes.kind(store).span().start);
    let mut tests = Vec::new();
    for reference in scoping.get_resolved_references(symbol) {
        if !reference.is_write() {
            continue;
        }
        let at = nodes.kind(reference.node_id()).span().start;
        if at < start || at >= end {
            continue;
        }
        for ancestor in nodes.ancestor_ids(reference.node_id()) {
            if ancestor == body {
                break;
            }
            if let AstKind::IfStatement(statement) = nodes.kind(ancestor) {
                let span = statement.test.span();
                if !tests.contains(&span) {
                    tests.push(span);
                }
            }
        }
    }
    tests
}

/// The values stored into a list held as element `i` of a register tuple
/// (`t = [k, list]; t[i][t[0]] = v`, the tracer's packed registers): the
/// reference at `reference` is that element. Only a tuple binding declared
/// once from the array (never reassigned) is followed.
fn tuple_element_stores(ctx: &FinderCtx, reference: oxc_semantic::NodeId) -> Vec<oxc_span::Span> {
    use oxc_ast::ast::{AssignmentTarget, BindingPattern, Expression};
    use oxc_ast::AstKind;
    use oxc_span::GetSpan;
    let nodes = ctx.parse.nodes;
    let scoping = ctx.parse.scoping;
    let array_id = nodes.parent_id(reference);
    let AstKind::ArrayExpression(array) = nodes.kind(array_id) else {
        return Vec::new();
    };
    let span = nodes.kind(reference).span();
    let Some(index) = array
        .elements
        .iter()
        .position(|item| item.as_expression().is_some_and(|expression| expression.span() == span))
    else {
        return Vec::new();
    };
    let AstKind::VariableDeclarator(declarator) = nodes.kind(nodes.parent_id(array_id)) else {
        return Vec::new();
    };
    let BindingPattern::BindingIdentifier(identifier) = &declarator.id else {
        return Vec::new();
    };
    let Some(tuple) = identifier.symbol_id.get() else {
        return Vec::new();
    };
    if scoping.symbol_is_mutated(tuple) {
        return Vec::new();
    }
    let mut out = Vec::new();
    for read in scoping.get_resolved_references(tuple) {
        // `t[i]`, the list ...
        let element = nodes.parent_id(read.node_id());
        let AstKind::ComputedMemberExpression(element_member) = nodes.kind(element) else { continue };
        if !matches!(element_member.expression.without_parentheses(), Expression::NumericLiteral(n) if n.value == index as f64) {
            continue;
        }
        // ... indexed and assigned: `t[i][…] = v`.
        let mut store = nodes.parent_id(element);
        let AstKind::ComputedMemberExpression(store_member) = nodes.kind(store) else { continue };
        if !matches!(store_member.object.without_parentheses(), Expression::ComputedMemberExpression(object) if object.span == element_member.span) {
            continue;
        }
        let store_span = store_member.span;
        let mut assignment = nodes.parent_id(store);
        if !matches!(nodes.kind(assignment), AstKind::AssignmentExpression(_)) {
            store = assignment;
            assignment = nodes.parent_id(store);
        }
        if let AstKind::AssignmentExpression(assign) = nodes.kind(assignment) {
            if assign.operator.is_assign()
                && matches!(&assign.left, AssignmentTarget::ComputedMemberExpression(target) if target.span == store_span)
            {
                out.push(assign.right.span());
                out.extend(choosing_tests(ctx, assignment, &assign.right));
            }
        }
    }
    out
}

/// The values pushed through a `push` method read out of its list at
/// `member` (`list.push`): bound to `p` (`p.call(_, v…)`) or held as element
/// 1 of a tuple `t = [list, list.push]` (`t[1].call(t[0], v…)`). Only a
/// binding declared once from that value is followed.
pub(crate) fn extracted_push_values(ctx: &FinderCtx, member: oxc_semantic::NodeId) -> Vec<oxc_span::Span> {
    use oxc_ast::ast::{BindingPattern, Expression};
    use oxc_ast::AstKind;
    use oxc_span::GetSpan;
    let nodes = ctx.parse.nodes;
    let scoping = ctx.parse.scoping;
    let mut holder = nodes.parent_id(member);
    let mut element: Option<usize> = None;
    if let AstKind::ArrayExpression(array) = nodes.kind(holder) {
        let span = nodes.kind(member).span();
        element = array
            .elements
            .iter()
            .position(|item| item.as_expression().is_some_and(|expression| expression.span() == span));
        if element.is_none() {
            return Vec::new();
        }
        holder = nodes.parent_id(holder);
    }
    // `let p = list.push`, or the tracer's hoisted `let p; … p = list.push`
    // when that assignment is the binding's only write.
    let binding = match nodes.kind(holder) {
        AstKind::VariableDeclarator(declarator) => {
            let BindingPattern::BindingIdentifier(identifier) = &declarator.id else {
                return Vec::new();
            };
            let Some(binding) = identifier.symbol_id.get() else {
                return Vec::new();
            };
            if scoping.symbol_is_mutated(binding) {
                return Vec::new();
            }
            binding
        }
        AstKind::AssignmentExpression(assignment) if assignment.operator.is_assign() => {
            let oxc_ast::ast::AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else {
                return Vec::new();
            };
            let Some(binding) = target.reference_id.get().and_then(|id| scoping.get_reference(id).symbol_id()) else {
                return Vec::new();
            };
            let declared_bare = matches!(
                nodes.kind(scoping.symbol_declaration(binding)),
                AstKind::VariableDeclarator(declarator) if declarator.init.is_none()
            );
            let writes = scoping.get_resolved_references(binding).filter(|reference| reference.is_write()).count();
            if !declared_bare || writes != 1 {
                return Vec::new();
            }
            binding
        }
        _ => return Vec::new(),
    };
    let mut out = Vec::new();
    for reference in scoping.get_resolved_references(binding) {
        let mut method = reference.node_id();
        if let Some(index) = element {
            let read = nodes.parent_id(method);
            match nodes.kind(read) {
                AstKind::ComputedMemberExpression(read_member)
                    if matches!(read_member.expression.without_parentheses(), Expression::NumericLiteral(n) if n.value == index as f64) =>
                {
                    method = read;
                }
                _ => continue,
            }
        }
        // Handed to an inline helper with the list
        // (`((l, p, d, k) => { p.call(l, d[k]); })(list, push, data, c)`).
        if let Some(values) = inline_helper_push_values(ctx, method) {
            out.extend(values);
            continue;
        }
        let call_member = nodes.parent_id(method);
        let AstKind::StaticMemberExpression(call_member_expression) = nodes.kind(call_member) else { continue };
        if call_member_expression.property.name != "call" {
            continue;
        }
        let AstKind::CallExpression(call) = nodes.kind(nodes.parent_id(call_member)) else { continue };
        if !matches!(call.callee.without_parentheses(), Expression::StaticMemberExpression(callee) if callee.span == call_member_expression.span) {
            continue;
        }
        out.extend(call.arguments.iter().skip(1).filter_map(|argument| argument.as_expression().map(|e| e.span())));
    }
    out
}

/// The values an inline helper pushes through the push method passed as
/// the argument at `method`: in `((l, p, d, k) => { p.call(l, d[k]); })(list,
/// push, data, c)` the helper calls its formal `p` with `.call`, and each
/// pushed value stands for the caller arguments of the formals it reads
/// (`data`, `c`), or for itself when it reads none. `None` when `method` is
/// not such an argument.
fn inline_helper_push_values(ctx: &FinderCtx, method: oxc_semantic::NodeId) -> Option<Vec<oxc_span::Span>> {
    use oxc_ast::ast::{BindingPattern, Expression};
    use oxc_ast::AstKind;
    use oxc_span::GetSpan;
    let nodes = ctx.parse.nodes;
    let scoping = ctx.parse.scoping;
    let span = nodes.kind(method).span();
    // The argument may sit in parentheses.
    let mut argument = method;
    loop {
        let parent = nodes.parent_id(argument);
        match nodes.kind(parent) {
            AstKind::ParenthesizedExpression(_) => argument = parent,
            AstKind::CallExpression(_) => break,
            _ => return None,
        }
    }
    let AstKind::CallExpression(call) = nodes.kind(nodes.parent_id(argument)) else { return None };
    let position = call.arguments.iter().position(|item| {
        item.as_expression().is_some_and(|expression| expression.without_parentheses().span() == span)
    })?;
    let params = match call.callee.without_parentheses() {
        Expression::ArrowFunctionExpression(arrow) => &arrow.params,
        Expression::FunctionExpression(function) => &function.params,
        _ => return None,
    };
    let formal = |item: &oxc_ast::ast::FormalParameter<'_>| match &item.pattern {
        BindingPattern::BindingIdentifier(identifier) => identifier.symbol_id.get(),
        _ => None,
    };
    let formals: Vec<Option<oxc_semantic::SymbolId>> = params.items.iter().map(formal).collect();
    let pusher = (*formals.get(position)?)?;
    if scoping.symbol_is_mutated(pusher) {
        return Some(Vec::new());
    }
    let mut out = Vec::new();
    for reference in scoping.get_resolved_references(pusher) {
        let call_member = nodes.parent_id(reference.node_id());
        let AstKind::StaticMemberExpression(member) = nodes.kind(call_member) else { continue };
        if member.property.name != "call" {
            continue;
        }
        let AstKind::CallExpression(push) = nodes.kind(nodes.parent_id(call_member)) else { continue };
        if !matches!(push.callee.without_parentheses(), Expression::StaticMemberExpression(callee) if callee.span == member.span) {
            continue;
        }
        for value in push.arguments.iter().skip(1).filter_map(|item| item.as_expression()) {
            let value_span = value.span();
            let mut read_formals = Vec::new();
            for (index, symbol) in formals.iter().enumerate() {
                let Some(symbol) = symbol else { continue };
                if scoping.get_resolved_references(*symbol).any(|read| {
                    let at = nodes.kind(read.node_id()).span();
                    value_span.start <= at.start && at.end <= value_span.end
                }) {
                    read_formals.push(index);
                }
            }
            if read_formals.is_empty() {
                out.push(value_span);
            } else {
                out.extend(
                    read_formals
                        .into_iter()
                        .filter_map(|index| call.arguments.get(index)?.as_expression().map(|e| e.span())),
                );
            }
        }
    }
    Some(out)
}

/// Facts of the backward slices of the `if` tests enclosing the slot's
/// write sites within their function (each definition followed,
/// [`MAX_EVIDENCE_DEPTH`] deep): a record written
/// when a step fails (`s = gl.createShader(…); if (s == null) write(…)`)
/// carries its evidence in the guards, not in its literal values.
pub(crate) fn guard_slices(view: &SlotView, ctx: &FinderCtx) -> LocalFacts {
    use oxc_ast::AstKind;
    use oxc_span::GetSpan;
    let nodes = ctx.parse.nodes;
    let mut tests = std::collections::BTreeSet::new();
    for leaf in view.leaves() {
        // The `if` tests around the write's own site, up to its function.
        let Some(mut id) = ctx.node_with_span(leaf.write.site()) else { continue };
        loop {
            let parent = nodes.parent_id(id);
            if parent == id {
                break;
            }
            id = parent;
            match nodes.kind(id) {
                AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) => break,
                AstKind::IfStatement(statement) => {
                    if let Some(test) = ctx.node_with_span(statement.test.span()) {
                        tests.insert(test);
                    }
                }
                _ => {}
            }
        }
    }
    let mut facts = LocalFacts::default();
    {
        for test in tests {
            let slice = ctx.slice_of(test, MAX_EVIDENCE_DEPTH);
            for (member, depth) in slice.members {
                let entry = facts.members.entry(member).or_insert(depth);
                *entry = (*entry).min(depth);
            }
            for (call, depth) in slice.calls {
                let entry = facts.calls.entry(call).or_insert(depth);
                *entry = (*entry).min(depth);
            }
        }
    }
    facts
}
