//! What each written value of a slot is made of ([`Desc`]), flow-aware,
//! plus the listeners and timers that touch the slot's state ([`SlotFacts`]).
//!
//! The leaf provenance of the [`SlotView`] is flow-insensitive and shared
//! registers blur it, so this builds a description of every written value
//! from the parse:
//!
//! * the slot's write sites are the anchors that are writer calls taking an
//!   emission buffer (a binding initialised with the 4-element raw prefix) or
//!   indexed stores into one; they are matched to the emission leaves in
//!   evaluation order;
//! * a written value is expanded through the nearest preceding definition of
//!   every local binding in the same function (all definitions for closure
//!   state), collecting host member names, calls, operators, `x * x` squares,
//!   record labels (`rec.k` of an element read), update counters, the
//!   lifecycle callback of each state definition and the comparison guarding
//!   it (running min/max, throttles);
//! * object literals of the slot spans give the meaning of record labels;
//! * event listeners and timers whose handler touches the slot's state.
//!
//! The facts computed by a claiming recognizer are memoized per slot in the
//! analysis' [`FinderCtx::memo`] and read back by the binder of the same slot.
//!
//! Used by: `AnimationFrameTimings`, `CurrentScriptAsyncPolicyState`,
//! `DeviceOrientation`, `IntervalClockDriftStats`, `KeyboardEventSamples`,
//! `KeystrokeDynamics`, `LifecycleEntropySamples`, `LifecyclePhaseTimings`,
//! `LocalStoragePersistentId`, `MouseTravelAndClickCount`,
//! `NavigationTimingOffsets`, `PasteCountUnfocusedInputFlag`,
//! `PersistentStorageState`, `PonosXhrRoundTrip`, `ScrollOffsets`,
//! `TimestampsEpoch`, `UiEventSamplesMouse`, `UiEventSamplesTouch`,
//! `UiEventSamplesWheel`, `VisibilityEventLog`; helper modules
//! [`described_fields`](super::described_fields).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::rc::Rc;

use oxc_ast::ast::{
    Argument, AssignmentTarget, ChainElement, Expression, ObjectPropertyKind,
    SimpleAssignmentTarget,
};
use oxc_ast::AstKind;
use oxc_semantic::{NodeId, SymbolId};
use oxc_span::GetSpan;

use crate::signals::analysis::roles;
use crate::signals::extract::wiring::LifecycleRole;
use crate::signals::schema::SourceSpan;
use crate::signals::analysis::slot_view::generated;
use crate::signals::analysis::{FinderCtx, LeafOp, SlotView};

/// What one written value is made of.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Desc {
    /// Static member names read (host and program).
    pub members: BTreeSet<String>,
    /// Members read off an element of a list (`list[i].k`): record labels.
    pub labels: Vec<String>,
    /// Called method / function names (`round`, `toFixed`, `now`, `getItem`).
    pub calls: BTreeSet<String>,
    /// A call of a program function (generated name) was seen.
    pub program_call: bool,
    /// Constructed names (`Date`, `XMLHttpRequest`).
    pub news: BTreeSet<String>,
    /// Unbound global names read.
    pub globals: BTreeSet<String>,
    pub ops: BTreeSet<String>,
    /// Atoms (members, calls, program calls) multiplied by themselves.
    pub squared: BTreeSet<String>,
    /// The value is a numeric literal (through plain copies).
    pub literal: Option<f64>,
    /// Anything other than literals and operators was read.
    pub nonliteral: bool,
    /// Some definition is an update (`++x`) or `x += 1`.
    pub counter: bool,
    pub conditional: bool,
    pub strings: BTreeSet<String>,
    /// Lifecycle callbacks holding the definitions of the written binding
    /// (`setup` for the producer body).
    pub phases: BTreeSet<&'static str>,
    /// How a definition of the written binding is guarded.
    pub guards: BTreeSet<Guard>,
    /// Offsets of every definition visited.
    pub def_sites: Vec<u32>,
    /// Bindings visited (state candidates for listener evidence).
    pub symbols: BTreeSet<SymbolId>,
    /// `rec.k[i]` reads: (label, literal index).
    pub indexed: Vec<(String, u32)>,
    /// Numeric literals seen (rendered).
    pub numbers: BTreeSet<String>,
    /// Receivers of method calls on the value's path (`ring.toArray()`),
    /// with their plain copies.
    pub containers: BTreeSet<SymbolId>,
}

/// An object literal (with the `holder.k = v` statements completing
/// it) and the receivers it is stored into (`ring.put(record)`).
#[derive(Clone, Debug, Default)]
pub(crate) struct RecordLiteral {
    pub fields: BTreeMap<String, Desc>,
    pub containers: BTreeSet<SymbolId>,
    /// Bindings stored as a field value as they are (`{k: list}`,
    /// `r.k = list`).
    pub held: BTreeSet<SymbolId>,
}

/// How a definition of a written binding is guarded.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Guard {
    /// Assigned only when the new value exceeds it (running maximum).
    Max,
    /// Assigned only when the new value is below it (running minimum).
    Min,
    /// Updated only when an elapsed difference exceeds a threshold.
    Throttle,
}

impl Desc {
    /// Add everything `other` describes.
    pub fn merge(&mut self, other: Desc) {
        self.members.extend(other.members);
        for label in other.labels {
            if !self.labels.contains(&label) {
                self.labels.push(label);
            }
        }
        self.calls.extend(other.calls);
        self.program_call |= other.program_call;
        self.news.extend(other.news);
        self.globals.extend(other.globals);
        self.ops.extend(other.ops);
        self.squared.extend(other.squared);
        self.nonliteral |= other.nonliteral;
        self.counter |= other.counter;
        self.conditional |= other.conditional;
        self.strings.extend(other.strings);
        self.phases.extend(other.phases);
        self.guards.extend(other.guards);
        self.def_sites.extend(other.def_sites);
        self.symbols.extend(other.symbols);
        for item in other.indexed {
            if !self.indexed.contains(&item) {
                self.indexed.push(item);
            }
        }
        self.numbers.extend(other.numbers);
        self.containers.extend(other.containers);
    }

    fn atoms(&self) -> BTreeSet<String> {
        let mut atoms: BTreeSet<String> = self.members.iter().cloned().collect();
        atoms.extend(self.calls.iter().cloned());
        if self.program_call {
            atoms.insert("()".into());
        }
        atoms
    }

    /// Member `name` is read.
    pub fn has_member(&self, name: &str) -> bool {
        self.members.contains(name)
    }

    /// `name` is called.
    pub fn has_call(&self, name: &str) -> bool {
        self.calls.contains(name)
    }

    /// Operator `op` is applied.
    pub fn has_op(&self, op: &str) -> bool {
        self.ops.contains(op)
    }

    /// The value reads nothing but literals.
    pub fn literal_only(&self) -> bool {
        !self.nonliteral && !self.counter
    }

    /// The value reads a clock: `Date`, `Date.now()`, `performance.now()`
    /// or a program clock function.
    pub fn reads_date(&self) -> bool {
        self.news.contains("Date") || (self.calls.contains("now") && self.globals.contains("Date"))
    }
}

/// Facts of one slot.
#[derive(Clone, Debug, Default)]
pub(crate) struct SlotFacts {
    /// Per emission of the view: the description of every leaf, in leaf
    /// order, when the write sites could be matched to the leaves.
    pub emissions: Vec<Option<Vec<Desc>>>,
    /// Event names of listeners whose handler touches the slot's state.
    pub events: BTreeSet<String>,
    /// Timer APIs (`requestAnimationFrame`, `setInterval`) whose callback
    /// touches the slot's state.
    pub timers: BTreeSet<String>,
    /// Per event or timer of [`Self::events`] / [`Self::timers`]: the
    /// static members its touching handler functions read.
    pub handler_members: BTreeMap<String, BTreeSet<String>>,
    /// Object literals of the slot spans: label -> value description.
    pub literals: Vec<RecordLiteral>,
    /// Pushes into the lists repeated leaves read: (lifecycle callback,
    /// pushed value).
    pub appends: Vec<(&'static str, Desc)>,
}

impl SlotFacts {
    /// Every leaf description of every matched emission.
    pub fn descs(&self) -> impl Iterator<Item = &Desc> {
        self.emissions.iter().flatten().flatten()
    }

    /// Every emission's write sites were matched to its leaves.
    pub fn all_matched(&self) -> bool {
        !self.emissions.is_empty() && self.emissions.iter().all(Option::is_some)
    }

    /// Some leaf reads member `name`.
    pub fn any_member(&self, name: &str) -> bool {
        self.descs().any(|desc| desc.has_member(name))
    }

    /// A listener for one of `names` touches the slot's state.
    pub fn has_event(&self, names: &[&str]) -> bool {
        names.iter().any(|name| self.events.contains(*name))
    }
}

// ---------------------------------------------------------------------------
// Per-program index and per-slot cache
// ---------------------------------------------------------------------------

struct ProgramIndex {
    /// Call, assignment and object-literal nodes by start offset.
    by_start: HashMap<u32, Vec<NodeId>>,
    /// Call expressions sorted by start.
    calls: Vec<(u32, NodeId)>,
    /// Object expressions sorted by start.
    objects: Vec<(u32, NodeId)>,
    /// Static member names read, by start.
    members: Vec<(u32, String)>,
}

/// Memo keys of this module (per analysis, in [`FinderCtx::memo`]).
#[derive(Hash, PartialEq, Eq)]
struct ProgramIndexKey;

#[derive(Hash, PartialEq, Eq)]
struct SlotKey {
    anchors: Vec<u32>,
    producer: Option<(u32, u32)>,
}

impl SlotKey {
    fn of(view: &SlotView) -> Self {
        Self {
            anchors: view.anchors().to_vec(),
            producer: view.producer_span().map(|span| (span.start, span.end)),
        }
    }
}

fn program_index(ctx: &FinderCtx) -> Rc<ProgramIndex> {
    ctx.memo(ProgramIndexKey, || build_program_index(ctx.parse))
}

fn build_program_index(parse: &roles::FinderCtx<'_, '_>) -> ProgramIndex {
    let mut by_start: HashMap<u32, Vec<NodeId>> = HashMap::new();
    let mut calls = Vec::new();
    let mut objects = Vec::new();
    let mut members = Vec::new();
    for (id, node) in parse.nodes.iter_enumerated() {
        match node.kind() {
            AstKind::CallExpression(call) => {
                by_start.entry(call.span.start).or_default().push(id);
                calls.push((call.span.start, id));
            }
            AstKind::AssignmentExpression(assignment) => {
                by_start.entry(assignment.span.start).or_default().push(id);
            }
            AstKind::ObjectExpression(object) => objects.push((object.span.start, id)),
            AstKind::StaticMemberExpression(member) => members.push((member.span.start, member.property.name.to_string())),
            _ => {}
        }
    }
    calls.sort_unstable();
    objects.sort_unstable();
    members.sort_unstable();
    ProgramIndex {
        by_start,
        calls,
        objects,
        members,
    }
}

/// The facts of a slot, computed once per slot and analysis (binders read
/// the facts their recognizer built).
pub(crate) fn slot_facts(view: &SlotView, ctx: &FinderCtx) -> Rc<SlotFacts> {
    ctx.memo(SlotKey::of(view), || {
        let index = program_index(ctx);
        Builder::new(view, ctx.parse, &index).build()
    })
}

// ---------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------

struct Def<'a> {
    start: u32,
    /// Where the definition takes effect (after its right-hand side).
    end: u32,
    node: NodeId,
    func: Option<NodeId>,
    rhs: Option<&'a Expression<'a>>,
    update: bool,
}

struct Builder<'v, 'x, 'a> {
    view: &'v SlotView,
    parse: &'x roles::FinderCtx<'x, 'a>,
    index: &'x ProgramIndex,
    spans: Vec<SourceSpan>,
    budget: usize,
    visiting: Vec<(SymbolId, u32)>,
    /// Listener / timer handler spans touching the slot's state.
    handlers: Vec<SourceSpan>,
    /// Lifecycle callbacks: the proven registrations, else the callbacks
    /// passed to the producer's own registrar parameters by position.
    callbacks: Vec<(LifecycleRole, SourceSpan)>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum WriteKind {
    Call,
    Store,
    /// The emission call sending the buffer (not a value write).
    Site,
}

struct Write<'a> {
    key: u32,
    kind: WriteKind,
    buffer: SymbolId,
    value: &'a Expression<'a>,
    at: u32,
    func: Option<NodeId>,
}

fn strip<'e, 'a>(expression: &'e Expression<'a>) -> &'e Expression<'a> {
    expression.without_parentheses()
}


impl<'v, 'x, 'a> Builder<'v, 'x, 'a> {
    fn new(view: &'v SlotView, parse: &'x roles::FinderCtx<'x, 'a>, index: &'x ProgramIndex) -> Self {
        Self {
            view,
            parse,
            index,
            spans: view.spans(),
            budget: 0,
            visiting: Vec::new(),
            handlers: Vec::new(),
            callbacks: match view.callback_spans() {
                [] => positional_callbacks(view, parse, index),
                proven => proven.to_vec(),
            },
        }
    }

    fn inside_slot(&self, offset: u32) -> bool {
        self.spans.iter().any(|span| offset >= span.start && offset < span.end)
    }

    fn function_of(&self, node: NodeId) -> Option<NodeId> {
        self.parse.nodes.ancestor_ids(node).find(|id| {
            matches!(
                self.parse.nodes.kind(*id),
                AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)
            )
        })
    }

    /// `t[k]` where the one definition of `t` reaching `at` is an array
    /// literal: its element `k`; otherwise `expression`.
    fn through_tuple(&self, expression: &'a Expression<'a>, at: u32, func: Option<NodeId>) -> &'a Expression<'a> {
        let Expression::ComputedMemberExpression(member) = strip(expression) else {
            return expression;
        };
        let Expression::NumericLiteral(index) = strip(&member.expression) else {
            return expression;
        };
        let Some(tuple) = self.symbol_of(&member.object) else {
            return expression;
        };
        match self.reaching(tuple, at, func).as_slice() {
            [def] if !def.update => match def.rhs.map(strip) {
                Some(Expression::ArrayExpression(array)) if index.value.fract() == 0.0 && index.value >= 0.0 => array
                    .elements
                    .get(index.value as usize)
                    .and_then(|element| element.as_expression())
                    .unwrap_or(expression),
                _ => expression,
            },
            _ => expression,
        }
    }

    fn symbol_of(&self, expression: &Expression<'a>) -> Option<SymbolId> {
        crate::analysis::expression_symbol(strip(expression), self.parse.scoping)
    }

    // ---- definitions ----

    fn defs(&self, symbol: SymbolId) -> Vec<Def<'a>> {
        let nodes = self.parse.nodes;
        let mut out = Vec::new();
        let declaration = self.parse.scoping.symbol_declaration(symbol);
        if let AstKind::VariableDeclarator(declarator) = nodes.kind(declaration) {
            if let Some(init) = &declarator.init {
                out.push(Def {
                    start: declarator.span.start,
                    end: declarator.span.end,
                    node: declaration,
                    func: self.function_of(declaration),
                    rhs: Some(init),
                    update: false,
                });
            }
        }
        for reference in self.parse.scoping.get_resolved_references(symbol) {
            if !reference.is_write() {
                continue;
            }
            let node = reference.node_id();
            let mut parent = nodes.parent_id(node);
            if !matches!(nodes.kind(parent), AstKind::AssignmentExpression(_) | AstKind::UpdateExpression(_)) {
                parent = nodes.parent_id(parent);
            }
            match nodes.kind(parent) {
                AstKind::AssignmentExpression(assignment) => {
                    let compound = assignment.operator.as_str() != "=";
                    let one = matches!(strip(&assignment.right), Expression::NumericLiteral(n) if n.value == 1.0);
                    out.push(Def {
                        start: assignment.span.start,
                        end: assignment.span.end,
                        node: parent,
                        func: self.function_of(parent),
                        rhs: Some(&assignment.right),
                        update: compound && one && assignment.operator.as_str() == "+=",
                    });
                }
                AstKind::UpdateExpression(update) => out.push(Def {
                    start: update.span.start,
                    end: update.span.end,
                    node: parent,
                    func: self.function_of(parent),
                    rhs: None,
                    update: true,
                }),
                _ => {}
            }
        }
        out.sort_by_key(|def| def.start);
        out
    }

    /// The definitions reaching a read at `at` in function `func`: the
    /// nearest preceding definition in the same function, else all.
    fn reaching(&self, symbol: SymbolId, at: u32, func: Option<NodeId>) -> Vec<Def<'a>> {
        let mut defs = self.defs(symbol);
        defs.sort_by_key(|def| def.end);
        let precedes = |def: &Def<'a>| def.func == func && def.end <= at && !self.exclusive(def.node, at);
        // Walk back from the read: assignments in branches that do not
        // hold the read may or may not have run, so keep collecting
        // until an assignment that always runs before the read.
        let mut chosen: Vec<Def<'a>> = Vec::new();
        let mut update = false;
        let mut settled = false;
        // An `if` before the read that assigns on every path
        // (`if (c) { r = a; } else { r = b; }`): the read sees one of the
        // assignments inside it, nothing from before it.
        let mut cutoff: Option<u32> = None;
        for index in (0..defs.len()).rev() {
            if cutoff.is_some_and(|start| defs[index].end <= start) && precedes(&defs[index]) {
                settled = true;
                break;
            }
            if !precedes(&defs[index]) {
                continue;
            }
            if defs[index].update {
                update = true;
                continue;
            }
            let conditional = self.conditional(defs[index].node, at);
            if conditional && cutoff.is_none() {
                cutoff = self.assigning_branch(defs[index].node, at, symbol);
            }
            chosen.push(defs.swap_remove(index));
            if !conditional {
                settled = true;
                break;
            }
        }
        if cutoff.is_some() && !chosen.is_empty() {
            // Every path through the branch assigns: nothing older reaches.
            settled = true;
        }
        if chosen.is_empty() {
            defs.sort_by_key(|def| def.start);
            return defs;
        }
        if !settled {
            // No assignment in this function always precedes the read:
            // the value on entry is any other definition.
            chosen.extend(defs.into_iter().filter(|def| def.func != func));
        }
        if update {
            // An update after the nearest assignment keeps it a counter.
            chosen.push(Def {
                start: at,
                end: at,
                node: chosen[0].node,
                func,
                rhs: None,
                update: true,
            });
        }
        chosen
    }

    /// The start of the outermost branch around `def` that does not hold
    /// the read at `at`, when that branch is an `if` assigning `symbol` on
    /// every path.
    fn assigning_branch(&self, def: NodeId, at: u32, symbol: SymbolId) -> Option<u32> {
        let nodes = self.parse.nodes;
        let mut outer: Option<NodeId> = None;
        for ancestor in nodes.ancestor_ids(def) {
            let kind = nodes.kind(ancestor);
            match kind {
                AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) => break,
                AstKind::IfStatement(_)
                | AstKind::ConditionalExpression(_)
                | AstKind::LogicalExpression(_)
                | AstKind::TryStatement(_)
                | AstKind::CatchClause(_)
                | AstKind::ForStatement(_)
                | AstKind::ForInStatement(_)
                | AstKind::ForOfStatement(_)
                | AstKind::WhileStatement(_)
                | AstKind::DoWhileStatement(_)
                | AstKind::SwitchCase(_) => {
                    let span = kind.span();
                    if at >= span.start && at < span.end {
                        break;
                    }
                    outer = Some(ancestor);
                }
                _ => {}
            }
        }
        let AstKind::IfStatement(statement) = nodes.kind(outer?) else {
            return None;
        };
        (statement.span.end <= at && self.always_assigns_if(statement, symbol)).then_some(statement.span.start)
    }

    fn always_assigns_if(&self, statement: &oxc_ast::ast::IfStatement<'a>, symbol: SymbolId) -> bool {
        statement.alternate.as_ref().is_some_and(|alternate| {
            self.always_assigns(&statement.consequent, symbol) && self.always_assigns(alternate, symbol)
        })
    }

    /// Every path through `statement` that falls through assigns `symbol`
    /// with `=` (straight-line statements, blocks and two-armed `if`s
    /// only).
    fn always_assigns(&self, statement: &oxc_ast::ast::Statement<'a>, symbol: SymbolId) -> bool {
        use oxc_ast::ast::Statement;
        match statement {
            Statement::BlockStatement(block) => block.body.iter().any(|inner| self.always_assigns(inner, symbol)),
            Statement::IfStatement(inner) => self.always_assigns_if(inner, symbol),
            Statement::ExpressionStatement(expression) => match strip(&expression.expression) {
                Expression::AssignmentExpression(assignment) => {
                    assignment.operator.as_str() == "="
                        && matches!(&assignment.left, AssignmentTarget::AssignmentTargetIdentifier(identifier)
                            if crate::analysis::identifier_symbol(identifier, self.parse.scoping) == Some(symbol))
                }
                _ => false,
            },
            _ => false,
        }
    }

    /// The definition sits in a branch, loop or `try` that does not
    /// hold the read at `at`.
    fn conditional(&self, def: NodeId, at: u32) -> bool {
        let nodes = self.parse.nodes;
        for ancestor in nodes.ancestor_ids(def) {
            let kind = nodes.kind(ancestor);
            let span = match kind {
                AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) => return false,
                AstKind::IfStatement(_)
                | AstKind::ConditionalExpression(_)
                | AstKind::LogicalExpression(_)
                | AstKind::TryStatement(_)
                | AstKind::CatchClause(_)
                | AstKind::ForStatement(_)
                | AstKind::ForInStatement(_)
                | AstKind::ForOfStatement(_)
                | AstKind::WhileStatement(_)
                | AstKind::DoWhileStatement(_)
                | AstKind::SwitchCase(_) => kind.span(),
                _ => continue,
            };
            if !(at >= span.start && at < span.end) {
                return true;
            }
        }
        false
    }

    /// The definition and the read sit in different arms of one branch.
    fn exclusive(&self, def: NodeId, at: u32) -> bool {
        let nodes = self.parse.nodes;
        let inside = |span: oxc_span::Span, offset: u32| offset >= span.start && offset < span.end;
        let start = nodes.get_node(def).span().start;
        for ancestor in nodes.ancestor_ids(def) {
            let (consequent, alternate) = match nodes.kind(ancestor) {
                AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) => return false,
                AstKind::IfStatement(statement) => {
                    (statement.consequent.span(), statement.alternate.as_ref().map(GetSpan::span))
                }
                AstKind::ConditionalExpression(conditional) => {
                    (conditional.consequent.span(), Some(conditional.alternate.span()))
                }
                _ => continue,
            };
            let Some(alternate) = alternate else {
                continue;
            };
            if (inside(consequent, start) && inside(alternate, at))
                || (inside(alternate, start) && inside(consequent, at))
            {
                return true;
            }
        }
        false
    }

    fn phase_of(&self, offset: u32) -> &'static str {
        let callback = self
            .callbacks
            .iter()
            .filter(|(_, span)| offset >= span.start && offset < span.end)
            .min_by_key(|(_, span)| span.end - span.start);
        match callback {
            Some((LifecycleRole::Init, _)) => "init",
            Some((LifecycleRole::Collection, _)) => "collection",
            Some((LifecycleRole::LastChance, _)) => "last_chance",
            Some((LifecycleRole::Cleanup, _)) => "cleanup",
            Some(_) => "callback",
            None => {
                if self.view.producer_span().is_some_and(|span| offset >= span.start && offset < span.end) {
                    "setup"
                } else {
                    "outer"
                }
            }
        }
    }

    /// The expression is `symbol` or a copy of it reaching `at`.
    fn is_symbol(&self, expression: &Expression<'a>, symbol: SymbolId, at: u32, func: Option<NodeId>) -> bool {
        let Some(found) = self.symbol_of(expression) else {
            return false;
        };
        if found == symbol {
            return true;
        }
        let reaching = self.reaching(found, at, func);
        reaching.len() == 1
            && reaching[0]
                .rhs
                .is_some_and(|rhs| self.symbol_of(rhs) == Some(symbol))
    }

    fn guard_of(&self, def: &Def<'a>, symbol: SymbolId) -> Option<Guard> {
        let nodes = self.parse.nodes;
        let mut child = def.node;
        for ancestor in nodes.ancestor_ids(def.node) {
            match nodes.kind(ancestor) {
                AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) => return None,
                AstKind::IfStatement(statement) => {
                    let in_consequent = {
                        let span = statement.consequent.span();
                        let child_span = nodes.get_node(child).span();
                        child_span.start >= span.start && child_span.end <= span.end
                    };
                    if in_consequent {
                        if let Expression::BinaryExpression(test) = strip(&statement.test) {
                            let op = test.operator.as_str();
                            let at = statement.span.start;
                            let left = self.is_symbol(&test.left, symbol, at, def.func);
                            let right = self.is_symbol(&test.right, symbol, at, def.func);
                            match (op, left, right) {
                                (">" | ">=", false, true) | ("<" | "<=", true, false) => return Some(Guard::Max),
                                ("<" | "<=", false, true) | (">" | ">=", true, false) => return Some(Guard::Min),
                                (">" | ">=", false, false)
                                    if matches!(strip(&test.left), Expression::BinaryExpression(inner) if inner.operator.as_str() == "-") =>
                                {
                                    return Some(Guard::Throttle)
                                }
                                _ => {}
                            }
                        }
                    }
                }
                _ => {}
            }
            child = ancestor;
        }
        None
    }

    // ---- expansion ----

    fn describe(&mut self, expression: &'a Expression<'a>, at: u32, func: Option<NodeId>) -> Desc {
        self.budget = 4000;
        self.visiting.clear();
        let mut desc = Desc::default();
        // The written binding: its definitions' phases and guards.
        if let Some(symbol) = self.symbol_of(expression) {
            let mut frontier = vec![(symbol, at, func, 0usize)];
            while let Some((symbol, at, func, hop)) = frontier.pop() {
                for def in self.reaching(symbol, at, func) {
                    if let Some(guard) = self.guard_of(&def, symbol) {
                        desc.guards.insert(guard);
                    }
                    // A copy (`v = w`) takes the phases of what it copies
                    // (`let v = elapsed; write(s, v, …)` in a later
                    // callback is still the earlier phase's timestamp).
                    let copied = (hop < 2)
                        .then(|| def.rhs.and_then(|rhs| self.symbol_of(rhs)))
                        .flatten()
                        .filter(|next| !self.reaching(*next, def.start, def.func).is_empty());
                    match copied {
                        Some(next) => frontier.push((next, def.start, def.func, hop + 1)),
                        None => {
                            desc.phases.insert(self.phase_of(def.start));
                        }
                    }
                }
            }
        }
        desc.literal = self.literal_value(expression, at, func, 0);
        self.expr(expression, at, func, 0, &mut desc);
        desc
    }

    fn literal_value(&self, expression: &'a Expression<'a>, at: u32, func: Option<NodeId>, hop: usize) -> Option<f64> {
        match strip(expression) {
            Expression::NumericLiteral(literal) => Some(literal.value),
            Expression::UnaryExpression(unary) if unary.operator.as_str() == "-" => {
                self.literal_value(&unary.argument, at, func, hop).map(|v| -v)
            }
            Expression::Identifier(_) if hop < 3 => {
                let symbol = self.symbol_of(expression)?;
                let defs = self.reaching(symbol, at, func);
                let values: Vec<Option<f64>> = defs
                    .iter()
                    .map(|def| def.rhs.and_then(|rhs| self.literal_value(rhs, def.start, def.func, hop + 1)))
                    .collect();
                match values.as_slice() {
                    [Some(value)] => Some(*value),
                    [first, rest @ ..] if first.is_some() && rest.iter().all(|v| v == first) => *first,
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn expr(&mut self, expression: &'a Expression<'a>, at: u32, func: Option<NodeId>, depth: usize, desc: &mut Desc) {
        if self.budget == 0 || depth > 40 {
            desc.nonliteral = true;
            return;
        }
        self.budget -= 1;
        match strip(expression) {
            Expression::NumericLiteral(literal) => {
                desc.numbers.insert(format!("{}", literal.value));
            }
            Expression::BooleanLiteral(_) | Expression::NullLiteral(_) => {}
            Expression::StringLiteral(literal) => {
                desc.strings.insert(literal.value.to_string());
            }
            Expression::Identifier(identifier) => {
                let Some(symbol) = crate::analysis::identifier_symbol(identifier, self.parse.scoping) else {
                    if identifier.name != "undefined" && !generated(&identifier.name) {
                        desc.globals.insert(identifier.name.to_string());
                        desc.nonliteral = true;
                    }
                    return;
                };
                desc.symbols.insert(symbol);
                let defs = self.reaching(symbol, at, func);
                if defs.is_empty() {
                    // A parameter or a declaration without a value.
                    desc.nonliteral = true;
                    return;
                }
                let depth_before = self.visiting.len();
                for def in defs {
                    self.visiting.truncate(depth_before);
                    // Each definition is expanded once per chain.
                    if self.visiting.contains(&(symbol, def.start)) {
                        continue;
                    }
                    self.visiting.push((symbol, def.start));
                    desc.def_sites.push(def.start);
                    if def.update {
                        desc.counter = true;
                        continue;
                    }
                    if let Some(rhs) = def.rhs {
                        if matches!(strip(rhs), Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_)) {
                            desc.nonliteral = true;
                            continue;
                        }
                        self.expr(rhs, def.start, def.func, depth + 1, desc);
                    }
                }
                self.visiting.truncate(depth_before);
            }
            Expression::StaticMemberExpression(member) => {
                desc.nonliteral = true;
                let name = member.property.name.to_string();
                if let Some(value) = self.object_property(&member.object, &name, at, func) {
                    desc.members.insert(name);
                    self.expr(value, at, func, depth + 1, desc);
                    return;
                }
                if self.is_element(&member.object, at, func) && !desc.labels.contains(&name) {
                    desc.labels.push(name.clone());
                }
                desc.members.insert(name);
                self.expr(&member.object, at, func, depth + 1, desc);
            }
            Expression::ComputedMemberExpression(member) => {
                desc.nonliteral = true;
                match strip(&member.expression) {
                    Expression::StringLiteral(key) => {
                        desc.members.insert(key.value.to_string());
                        self.expr(&member.object, at, func, depth + 1, desc);
                    }
                    Expression::NumericLiteral(_) if !std::ptr::eq(self.through_tuple(expression, at, func), expression) => {
                        // A tuple element (`t = [buf, i, v]; .. t[2] ..`).
                        let element = self.through_tuple(expression, at, func);
                        self.expr(element, at, func, depth + 1, desc);
                    }
                    Expression::NumericLiteral(index) => {
                        let mut object = Desc::default();
                        self.expr(&member.object, at, func, depth + 1, &mut object);
                        if let Some(label) = object.labels.last() {
                            let item = (label.clone(), index.value as u32);
                            if !desc.indexed.contains(&item) {
                                desc.indexed.push(item);
                            }
                        }
                        desc.merge(object);
                    }
                    _ => {
                        self.expr(&member.expression, at, func, depth + 1, desc);
                        self.expr(&member.object, at, func, depth + 1, desc);
                    }
                }
            }
            Expression::ChainExpression(chain) => {
                desc.nonliteral = true;
                match &chain.expression {
                    ChainElement::CallExpression(call) => {
                        self.callee(&call.callee, at, func, depth, desc);
                        for argument in &call.arguments {
                            if let Some(argument) = argument.as_expression() {
                                self.expr(argument, at, func, depth + 1, desc);
                            }
                        }
                    }
                    ChainElement::StaticMemberExpression(member) => {
                        desc.members.insert(member.property.name.to_string());
                        self.expr(&member.object, at, func, depth + 1, desc);
                    }
                    ChainElement::ComputedMemberExpression(member) => {
                        self.expr(&member.object, at, func, depth + 1, desc);
                        self.expr(&member.expression, at, func, depth + 1, desc);
                    }
                    _ => {}
                }
            }
            Expression::CallExpression(call) => {
                desc.nonliteral = true;
                self.callee(&call.callee, at, func, depth, desc);
                for argument in &call.arguments {
                    if let Some(argument) = argument.as_expression() {
                        if matches!(strip(argument), Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_)) {
                            continue;
                        }
                        self.expr(argument, at, func, depth + 1, desc);
                    }
                }
            }
            Expression::NewExpression(new) => {
                desc.nonliteral = true;
                if let Some(name) = self.host_name(&new.callee, at, func) {
                    desc.news.insert(name);
                } else {
                    desc.program_call = true;
                }
                for argument in &new.arguments {
                    if let Some(argument) = argument.as_expression() {
                        self.expr(argument, at, func, depth + 1, desc);
                    }
                }
            }
            Expression::BinaryExpression(binary) => {
                let op = binary.operator.as_str();
                desc.ops.insert(op.to_string());
                if op == "*" {
                    let mut left = Desc::default();
                    let mut right = Desc::default();
                    let visiting = self.visiting.clone();
                    self.expr(&binary.left, at, func, depth + 1, &mut left);
                    self.visiting = visiting.clone();
                    self.expr(&binary.right, at, func, depth + 1, &mut right);
                    self.visiting = visiting;
                    let both: Vec<String> = left.atoms().intersection(&right.atoms()).cloned().collect();
                    desc.squared.extend(both);
                    desc.merge(left);
                    desc.merge(right);
                } else {
                    self.expr(&binary.left, at, func, depth + 1, desc);
                    self.expr(&binary.right, at, func, depth + 1, desc);
                }
            }
            Expression::LogicalExpression(logical) => {
                desc.ops.insert(logical.operator.as_str().to_string());
                self.expr(&logical.left, at, func, depth + 1, desc);
                self.expr(&logical.right, at, func, depth + 1, desc);
            }
            Expression::ConditionalExpression(conditional) => {
                desc.conditional = true;
                self.expr(&conditional.test, at, func, depth + 1, desc);
                self.expr(&conditional.consequent, at, func, depth + 1, desc);
                self.expr(&conditional.alternate, at, func, depth + 1, desc);
            }
            Expression::UnaryExpression(unary) => {
                desc.ops.insert(format!("unary{}", unary.operator.as_str()));
                self.expr(&unary.argument, at, func, depth + 1, desc);
            }
            Expression::UpdateExpression(update) => {
                desc.counter = true;
                if let SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier) = &update.argument {
                    if let Some(symbol) = crate::analysis::identifier_symbol(identifier, self.parse.scoping) {
                        desc.symbols.insert(symbol);
                    }
                }
            }
            Expression::AssignmentExpression(assignment) => {
                self.expr(&assignment.right, at, func, depth + 1, desc);
            }
            Expression::SequenceExpression(sequence) => {
                if let Some(last) = sequence.expressions.last() {
                    self.expr(last, at, func, depth + 1, desc);
                }
            }
            Expression::ArrayExpression(array) => {
                desc.ops.insert("array".into());
                for element in &array.elements {
                    if let Some(element) = element.as_expression() {
                        self.expr(element, at, func, depth + 1, desc);
                    }
                }
            }
            Expression::ObjectExpression(_) => {
                desc.nonliteral = true;
                desc.ops.insert("object".into());
            }
            Expression::TemplateLiteral(_) => {
                desc.nonliteral = true;
            }
            _ => {
                desc.nonliteral = true;
            }
        }
    }

    fn callee(&mut self, callee: &'a Expression<'a>, at: u32, func: Option<NodeId>, depth: usize, desc: &mut Desc) {
        match strip(callee) {
            Expression::StaticMemberExpression(member) => {
                let name = member.property.name.to_string();
                if name == "call" || name == "apply" {
                    match self.host_name(&member.object, at, func) {
                        Some(inner) => {
                            desc.calls.insert(inner);
                        }
                        None => desc.program_call = true,
                    }
                } else {
                    desc.calls.insert(name);
                    desc.containers.extend(self.aliases(&member.object, at, func));
                }
                self.expr(&member.object, at, func, depth + 1, desc);
            }
            Expression::Identifier(identifier) if !generated(&identifier.name) => {
                if crate::analysis::identifier_symbol(identifier, self.parse.scoping).is_none() {
                    desc.calls.insert(identifier.name.to_string());
                } else {
                    desc.program_call = true;
                }
            }
            other => match self.host_name(other, at, func) {
                Some(name) => {
                    desc.calls.insert(name);
                }
                None => desc.program_call = true,
            },
        }
    }

    /// A host name for a callee or constructor: a global name, a member
    /// name, or the member a local alias was assigned from
    /// (`D = context.global.Date`).
    fn host_name(&self, expression: &'a Expression<'a>, at: u32, func: Option<NodeId>) -> Option<String> {
        match strip(expression) {
            Expression::Identifier(identifier) => {
                match crate::analysis::identifier_symbol(identifier, self.parse.scoping) {
                    None if !generated(&identifier.name) => Some(identifier.name.to_string()),
                    None => None,
                    Some(symbol) => {
                        let defs = self.reaching(symbol, at, func);
                        let names: BTreeSet<String> = defs
                            .iter()
                            .filter_map(|def| def.rhs)
                            .filter_map(|rhs| match strip(rhs) {
                                Expression::StaticMemberExpression(member) => Some(member.property.name.to_string()),
                                Expression::Identifier(inner)
                                    if !generated(&inner.name)
                                        && crate::analysis::identifier_symbol(inner, self.parse.scoping).is_none() =>
                                {
                                    Some(inner.name.to_string())
                                }
                                _ => None,
                            })
                            .collect();
                        (names.len() == 1).then(|| names.into_iter().next().unwrap_or_default())
                    }
                }
            }
            Expression::StaticMemberExpression(member) => Some(member.property.name.to_string()),
            _ => None,
        }
    }

    /// The binding `expression` names and the bindings it is a plain
    /// copy of (`r = ring` -> {r, ring}).
    fn aliases(&self, expression: &'a Expression<'a>, at: u32, func: Option<NodeId>) -> BTreeSet<SymbolId> {
        let mut out = BTreeSet::new();
        let mut current = self.symbol_of(expression).map(|symbol| (symbol, at, func));
        let mut hops = 0;
        while let Some((symbol, at, func)) = current {
            if !out.insert(symbol) || hops > 3 {
                break;
            }
            hops += 1;
            let defs = self.reaching(symbol, at, func);
            current = match defs.as_slice() {
                [def] => def
                    .rhs
                    .and_then(|rhs| self.symbol_of(rhs))
                    .map(|next| (next, def.start, def.func)),
                _ => None,
            };
        }
        out
    }

    /// The receivers an object literal (node `object`, held in
    /// `holder` when assigned to a binding) is passed to as a method
    /// argument (`ring.put(record)`, `list.push({..})`).
    fn literal_containers(&self, object: NodeId, span: oxc_span::Span) -> BTreeSet<SymbolId> {
        let nodes = self.parse.nodes;
        let scoping = self.parse.scoping;
        let mut out = BTreeSet::new();
        let receiver_of = |argument: oxc_span::Span, node: NodeId, out: &mut BTreeSet<SymbolId>| {
            for ancestor in nodes.ancestor_ids(node).take(2) {
                if let AstKind::CallExpression(call) = nodes.kind(ancestor) {
                    if call.arguments.iter().any(|a| a.span() == argument) {
                        if let Expression::StaticMemberExpression(member) = strip(&call.callee) {
                            let func = self.function_of(ancestor);
                            out.extend(self.aliases(&member.object, call.span.start, func));
                        }
                    }
                    break;
                }
            }
        };
        receiver_of(span, object, &mut out);
        let holder = match nodes.kind(nodes.parent_id(object)) {
            AstKind::VariableDeclarator(declarator) => {
                declarator.id.get_binding_identifier().and_then(|binding| binding.symbol_id.get())
            }
            AstKind::AssignmentExpression(assignment) => match &assignment.left {
                AssignmentTarget::AssignmentTargetIdentifier(identifier) => {
                    crate::analysis::identifier_symbol(identifier, scoping)
                }
                _ => None,
            },
            _ => None,
        };
        if let Some(holder) = holder {
            let func = self.function_of(object);
            for reference in scoping.get_resolved_references(holder) {
                let node = reference.node_id();
                let reference_span = nodes.get_node(node).span();
                if reference_span.start > span.end && self.function_of(node) == func {
                    receiver_of(reference_span, node, &mut out);
                }
            }
        }
        out
    }

    /// `object` is a list element (`list[i]`, or a copy of one).
    fn is_element(&self, object: &'a Expression<'a>, at: u32, func: Option<NodeId>) -> bool {
        let element = |expression: &Expression<'a>| {
            matches!(strip(expression), Expression::ComputedMemberExpression(member)
                if !matches!(strip(&member.expression), Expression::NumericLiteral(_) | Expression::StringLiteral(_)))
        };
        if element(object) {
            return true;
        }
        let Some(symbol) = self.symbol_of(object) else {
            return false;
        };
        let defs = self.reaching(symbol, at, func);
        !defs.is_empty() && defs.iter().all(|def| def.rhs.is_some_and(|rhs| element(rhs)))
    }

    /// `object.name` where `object` is (a copy of) an object literal with a
    /// static property `name`.
    fn object_property(&self, object: &'a Expression<'a>, name: &str, at: u32, func: Option<NodeId>) -> Option<&'a Expression<'a>> {
        let symbol = self.symbol_of(object)?;
        let defs = self.reaching(symbol, at, func);
        let [def] = defs.as_slice() else {
            return None;
        };
        let Expression::ObjectExpression(literal) = strip(def.rhs?) else {
            return None;
        };
        literal.properties.iter().find_map(|property| match property {
            ObjectPropertyKind::ObjectProperty(property) if property.key.static_name().as_deref() == Some(name) => {
                Some(&property.value)
            }
            _ => None,
        })
    }

    // ---- write sites ----

    /// The binding is an emission buffer: initialised with a 4-element
    /// array (the raw seed prefix).
    fn is_buffer(&self, symbol: SymbolId) -> bool {
        self.is_buffer_in(symbol, false)
    }

    /// [`Self::is_buffer`]; `nested` when asked about an element of a
    /// candidate prefix literal (that literal's elements are not checked
    /// again).
    fn is_buffer_in(&self, symbol: SymbolId, nested: bool) -> bool {
        let defs = self.defs(symbol);
        defs.iter().any(|def| {
            def.rhs.is_some_and(|rhs| matches!(strip(rhs), Expression::ArrayExpression(array) if self.prefix_literal(array, symbol, nested)))
        }) || defs.iter().any(|def| {
            // A shorter literal (`[]`, `[r >>> 24, r >> 16 & 255, …]`)
            // whose remaining raw-prefix bytes are element stores.
            def.rhs.is_some_and(|rhs| matches!(strip(rhs), Expression::ArrayExpression(array)
                if array.elements.len() < 4 && self.prefix_filled_from(symbol, array.elements.len() as u32)))
        })
    }

    /// A 4-element literal that is a raw seed prefix, not the tracer's
    /// argument tuple of a register it reuses (`t = [w, v, buf, t]; t =
    /// t[0].call(void 0, t[1], t[2], t[3])`): a tuple carries the
    /// register's old value or a buffer binding as an element.
    fn prefix_literal(&self, array: &oxc_ast::ast::ArrayExpression<'a>, symbol: SymbolId, nested: bool) -> bool {
        array.elements.len() == 4
            && (nested
                || !array.elements.iter().filter_map(|element| element.as_expression()).any(|element| {
                    matches!(strip(element), Expression::Identifier(_))
                        && self
                            .symbol_of(element)
                            .is_some_and(|held| held == symbol || self.is_buffer_in(held, true))
                }))
    }

    /// The binding gets the rest of its raw prefix, `b[from] = ..`
    /// through `b[3] = ..`, by element stores.
    fn prefix_filled_from(&self, symbol: SymbolId, from: u32) -> bool {
        let nodes = self.parse.nodes;
        let mut indices = BTreeSet::new();
        for reference in self.parse.scoping.get_resolved_references(symbol) {
            let AstKind::ComputedMemberExpression(member) = nodes.kind(nodes.parent_id(reference.node_id())) else {
                continue;
            };
            let Expression::NumericLiteral(index) = strip(&member.expression) else {
                continue;
            };
            let target = nodes.parent_id(nodes.parent_id(reference.node_id()));
            let stored = matches!(nodes.kind(target), AstKind::AssignmentExpression(_))
                || matches!(nodes.kind(nodes.parent_id(target)), AstKind::AssignmentExpression(_));
            if stored && index.value < 4.0 {
                indices.insert(index.value as u32);
            }
        }
        (from..4).all(|index| indices.contains(&index))
    }

    /// The buffer a register holds at `at` when its only reaching
    /// definition copies a buffer (`r = buffer; write(v, r)`).
    fn alias_buffer(&self, symbol: SymbolId, at: u32, func: Option<NodeId>) -> Option<SymbolId> {
        match self.reaching(symbol, at, func).as_slice() {
            [def] if !def.update && def.func == func => def
                .rhs
                .and_then(|rhs| self.symbol_of(strip(rhs)))
                .filter(|buffer| *buffer != symbol && self.is_buffer(*buffer)),
            // A branch join (`if (…) { r = b1; } else { r = b2; }`):
            // every arm copies a buffer; the register holds the
            // message buffer of whichever arm ran (the first stands
            // for them).
            defs if defs.len() > 1 => {
                let buffers: Option<Vec<SymbolId>> = defs
                    .iter()
                    .map(|def| {
                        (!def.update && def.func == func)
                            .then(|| def.rhs.and_then(|rhs| self.symbol_of(strip(rhs))))
                            .flatten()
                            .filter(|buffer| *buffer != symbol && self.is_buffer(*buffer))
                    })
                    .collect();
                buffers.and_then(|buffers| buffers.first().copied())
            }
            _ => None,
        }
    }

    /// The binding's definitions reaching `at` in `func` are all
    /// assignments of something other than a buffer (a 4-element array
    /// or a copy of a buffer binding).
    fn overwritten_buffer(&self, symbol: SymbolId, at: u32, func: Option<NodeId>) -> bool {
        let reaching = self.reaching(symbol, at, func);
        let local: Vec<&Def<'a>> = reaching.iter().filter(|def| def.func == func && def.end <= at).collect();
        !local.is_empty()
            && local.iter().all(|def| {
                // An update (`++r`) holds a number, never a buffer.
                def.rhs.is_none_or(|rhs| match strip(rhs) {
                    Expression::ArrayExpression(array) => !self.prefix_literal(array, symbol, false),
                    // A copy of a buffer binding still holds a buffer.
                    Expression::Identifier(_) => self.symbol_of(rhs).is_none_or(|copied| copied != symbol && !self.is_buffer(copied)),
                    _ => true,
                })
            })
    }

    /// The buffer a payload binding holds: its one reaching definition
    /// in this function is `[[], buffer]`.
    fn payload_buffer(&self, expression: &'a Expression<'a>, at: u32, func: Option<NodeId>) -> Option<SymbolId> {
        let symbol = self.symbol_of(expression)?;
        let reaching = self.reaching(symbol, at, func);
        let [def] = reaching.as_slice() else {
            return None;
        };
        if def.func != func || def.update {
            return None;
        }
        let Expression::ArrayExpression(array) = strip(def.rhs?) else {
            return None;
        };
        let [Some(first), Some(second)] = [array.elements.first(), array.elements.get(1)] else {
            return None;
        };
        if array.elements.len() != 2 || !matches!(first.as_expression().map(strip), Some(Expression::ArrayExpression(_))) {
            return None;
        }
        let inner = self.symbol_of(second.as_expression()?)?;
        if self.is_buffer(inner) {
            Some(inner)
        } else {
            self.alias_buffer(inner, def.start, func)
        }
    }

    fn writes(&self) -> Vec<Write<'a>> {
        let nodes = self.parse.nodes;
        let mut out = Vec::new();
        let mut buffers: HashMap<SymbolId, bool> = HashMap::new();
        let mut is_buffer = |symbol: SymbolId, this: &Self| *buffers.entry(symbol).or_insert_with(|| this.is_buffer(symbol));
        let mut buffer = |symbol: SymbolId, this: &Self, at: u32, func: Option<NodeId>| {
            if is_buffer(symbol, this) {
                Some(symbol)
            } else {
                this.alias_buffer(symbol, at, func)
            }
        };
        for anchor in self.view.anchors() {
            if !self.inside_slot(*anchor) {
                continue;
            }
            for id in self.index.by_start.get(anchor).into_iter().flatten() {
                match nodes.kind(*id) {
                    AstKind::CallExpression(call) => {
                        let (at, func) = (call.span.start, self.function_of(*id));
                        // The tracer's argument tuple (`t = [v, buf, i];
                        // w(m, t[0], t[1], t[2])`) passes its elements.
                        let arguments: Vec<&'a Expression<'a>> = call
                            .arguments
                            .iter()
                            .filter_map(Argument::as_expression)
                            .map(|argument| self.through_tuple(argument, at, func))
                            .collect();
                        // An emission site `emitter.next([[], buffer])`
                        // closes the buffer's current message.
                        let sent = arguments.iter().find_map(|argument| match strip(argument) {
                            Expression::ArrayExpression(array) => array
                                .elements
                                .iter()
                                .filter_map(|element| element.as_expression())
                                .filter_map(|element| self.symbol_of(element))
                                .find_map(|symbol| buffer(symbol, self, at, func))
                                .map(|symbol| (symbol, *argument)),
                            // `p = [[], buffer]; next.call(recv, p)`.
                            Expression::Identifier(_) => self.payload_buffer(argument, at, func).map(|symbol| (symbol, *argument)),
                            _ => None,
                        });
                        if let Some((symbol, argument)) = sent {
                            out.push(Write {
                                key: call.span.start,
                                kind: WriteKind::Site,
                                buffer: symbol,
                                value: argument,
                                at: call.span.start,
                                func: self.function_of(*id),
                            });
                            continue;
                        }
                        // The last buffer argument (a register may hold
                        // a stream after an earlier buffer).
                        // A register that once held a buffer and now holds
                        // a writer's cursor (`b = write(s, v, buf, b)`) is
                        // not the buffer at this call.
                        let holds = |argument: &&'a Expression<'a>, this: &Self| {
                            this.symbol_of(argument).is_some_and(|symbol| !this.overwritten_buffer(symbol, at, func))
                        };
                        let candidates: Vec<usize> = arguments
                            .iter()
                            .enumerate()
                            .filter(|(_, argument)| {
                                self.symbol_of(argument).and_then(|symbol| buffer(symbol, self, at, func)).is_some()
                            })
                            .map(|(position, _)| position)
                            .collect();
                        let position = candidates
                            .iter()
                            .rev()
                            .copied()
                            .find(|position| holds(&arguments[*position], self))
                            .or_else(|| candidates.last().copied());
                        let Some(position) = position.filter(|p| *p > 0) else {
                            continue;
                        };
                        let Some(symbol) =
                            self.symbol_of(arguments[position]).and_then(|symbol| buffer(symbol, self, at, func))
                        else {
                            continue;
                        };
                        // Writes are ordered by when they execute: a
                        // call after its arguments, a store after its
                        // index and value.
                        out.push(Write {
                            key: call.span.end,
                            kind: WriteKind::Call,
                            buffer: symbol,
                            value: arguments[position - 1],
                            at: call.span.start,
                            func: self.function_of(*id),
                        });
                    }
                    AstKind::AssignmentExpression(assignment) => {
                        let AssignmentTarget::ComputedMemberExpression(target) = &assignment.left else {
                            continue;
                        };
                        // `b[0..3] = ..` fills the raw prefix, not a value.
                        if matches!(strip(&target.expression), Expression::NumericLiteral(index) if index.value < 4.0) {
                            continue;
                        }
                        // `t = [buf, i, v]; t[0][t[1]] = ..`: the tuple's
                        // element is the buffer.
                        let object = self.through_tuple(&target.object, assignment.span.start, self.function_of(*id));
                        let Some(symbol) = self
                            .symbol_of(object)
                            .and_then(|symbol| buffer(symbol, self, assignment.span.start, self.function_of(*id)))
                        else {
                            continue;
                        };
                        out.push(Write {
                            key: assignment.span.end,
                            kind: WriteKind::Store,
                            buffer: symbol,
                            value: unmask(&assignment.right),
                            at: assignment.span.start,
                            func: self.function_of(*id),
                        });
                    }
                    _ => {}
                }
            }
        }
        out.sort_by_key(|write| write.key);
        out.dedup_by_key(|write| write.key);
        out
    }

    fn build(mut self) -> SlotFacts {
        let writes = self.writes();
        // Group by buffer, in source order; an emission site of the
        // buffer ends its group (a buffer binding reused by two
        // emission copies yields two groups).
        let mut groups: Vec<(SymbolId, Vec<&Write<'a>>)> = Vec::new();
        let mut open: HashMap<SymbolId, usize> = HashMap::new();
        // Groups closed by an emission site of their buffer.
        let mut closed: BTreeSet<usize> = BTreeSet::new();
        for write in &writes {
            if write.kind == WriteKind::Site {
                if let Some(index) = open.remove(&write.buffer) {
                    closed.insert(index);
                }
                continue;
            }
            match open.get(&write.buffer) {
                Some(index) => groups[*index].1.push(write),
                None => {
                    open.insert(write.buffer, groups.len());
                    groups.push((write.buffer, vec![write]));
                }
            }
        }
        let mut used = vec![false; groups.len()];
        let mut emissions = Vec::new();
        for emission in self.view.emissions() {
            // Each leaf's own write when every leaf's logical value is a
            // write found here: groups follow buffer names, which a
            // branch join (`if (…) { … } else { … b = c; }; b[k] = …`)
            // splits from the leaf order.
            let by_value: Option<Vec<&Write<'a>>> = emission
                .leaves
                .iter()
                .map(|leaf| {
                    let mut found = writes
                        .iter()
                        .filter(|write| write.kind != WriteKind::Site && write.value.span() == leaf.write.value_span);
                    let first = found.next()?;
                    found.next().is_none().then_some(first)
                })
                .collect();
            if let Some(own) = by_value.filter(|own| !own.is_empty()) {
                let descs = own
                    .iter()
                    .map(|write| self.describe(write.value, write.at, write.func))
                    .collect::<Vec<Desc>>();
                emissions.push(Some(descs));
                continue;
            }
            let ops: Vec<LeafOp> = emission.leaves.iter().map(|leaf| leaf.op).collect();
            // Groups some emission site sends first: a buffer never
            // sent here is another slot's (or a helper's) writes.
            let mut sent_only: Vec<bool> = (0..groups.len()).map(|index| used[index] || !closed.contains(&index)).collect();
            let matched = match match_groups(&ops, &groups, &mut sent_only) {
                Some(chosen) => {
                    for index in &chosen {
                        used[*index] = true;
                    }
                    Some(chosen)
                }
                None => match_groups(&ops, &groups, &mut used),
            };
            let descs = matched.map(|chosen| {
                chosen
                    .iter()
                    .flat_map(|index| groups[*index].1.iter())
                    .map(|write| {
                        let (value, at, func) = (write.value, write.at, write.func);
                        self.describe(value, at, func)
                    })
                    .collect::<Vec<Desc>>()
            });
            emissions.push(descs);
        }
        let mut facts = SlotFacts {
            emissions,
            ..SlotFacts::default()
        };
        self.handlers = self.listeners(&mut facts);
        self.literals(&mut facts);
        self.appends(&mut facts);
        facts
    }

    /// `list.push(v)` of the producer-owned lists a repeated leaf reads:
    /// the lifecycle callback of each push and what `v` is made of.
    fn appends(&mut self, facts: &mut SlotFacts) {
        let nodes = self.parse.nodes;
        let scoping = self.parse.scoping;
        let mut lists: BTreeSet<SymbolId> = BTreeSet::new();
        for (emission, descs) in self.view.emissions().iter().zip(&facts.emissions) {
            let Some(descs) = descs else {
                continue;
            };
            for (leaf, desc) in emission.leaves.iter().zip(descs) {
                if leaf.repeat_depth > 0 {
                    lists.extend(desc.symbols.iter().copied());
                }
            }
        }
        let producer = self.view.producer_span();
        let mut pushes: Vec<(u32, &'a Expression<'a>, Option<NodeId>)> = Vec::new();
        for list in lists {
            let declared = nodes.get_node(scoping.symbol_declaration(list)).span().start;
            if !producer.is_some_and(|span| declared >= span.start && declared < span.end) {
                continue;
            }
            for reference in scoping.get_resolved_references(list) {
                let member = nodes.parent_id(reference.node_id());
                let AstKind::StaticMemberExpression(member_expression) = nodes.kind(member) else {
                    continue;
                };
                if !matches!(member_expression.property.name.as_str(), "push" | "unshift") {
                    continue;
                }
                let call = nodes.parent_id(member);
                let AstKind::CallExpression(call_expression) = nodes.kind(call) else {
                    continue;
                };
                if let Some(argument) = call_expression.arguments.first().and_then(Argument::as_expression) {
                    pushes.push((call_expression.span.start, argument, self.function_of(call)));
                }
            }
        }
        for (at, argument, func) in pushes {
            let phase = self.phase_of(at);
            let desc = self.describe(argument, at, func);
            facts.appends.push((phase, desc));
        }
    }

    /// Listener and timer registrations in the slot spans whose handler
    /// touches a binding of the slot's written values; returns the
    /// spans of those handlers.
    fn listeners(&self, facts: &mut SlotFacts) -> Vec<SourceSpan> {
        let mut handlers: Vec<SourceSpan> = Vec::new();
        let nodes = self.parse.nodes;
        let scoping = self.parse.scoping;
        let mut symbols: BTreeSet<SymbolId> = BTreeSet::new();
        let mut sites: Vec<u32> = self.view.anchors().to_vec();
        for desc in facts.descs() {
            symbols.extend(desc.symbols.iter().copied());
            sites.extend(desc.def_sites.iter().copied());
        }
        // Bindings that are called are helpers (clocks, writers), not state.
        let state: Vec<SymbolId> = symbols
            .iter()
            .copied()
            .filter(|symbol| {
                let declared = nodes.get_node(scoping.symbol_declaration(*symbol)).span().start;
                self.view.producer_span().is_some_and(|span| declared >= span.start && declared < span.end)
                    && !scoping.get_resolved_references(*symbol).any(|reference| {
                        matches!(nodes.kind(nodes.parent_id(reference.node_id())), AstKind::CallExpression(call)
                            if matches!(strip(&call.callee), Expression::Identifier(id) if id.span == nodes.get_node(reference.node_id()).span()))
                    })
            })
            .collect();
        let touched = |span: oxc_span::Span| {
            sites.iter().any(|site| *site >= span.start && *site < span.end)
                || state.iter().any(|symbol| {
                    scoping.get_resolved_references(*symbol).any(|reference| {
                        let at = nodes.get_node(reference.node_id()).span().start;
                        at >= span.start && at < span.end
                    })
                })
        };
        for span in &self.spans {
            let first = self.index.calls.partition_point(|(start, _)| *start < span.start);
            for (start, id) in &self.index.calls[first..] {
                if *start >= span.end {
                    break;
                }
                let AstKind::CallExpression(call) = nodes.kind(*id) else {
                    continue;
                };
                let arguments: Vec<&'a Expression<'a>> = call.arguments.iter().filter_map(Argument::as_expression).collect();
                let func = self.function_of(*id);
                let name = match strip(&call.callee) {
                    Expression::StaticMemberExpression(member) if member.property.name == "call" => {
                        self.host_name(&member.object, *start, func)
                    }
                    other => self.host_name(other, *start, func),
                };
                let Some(name) = name else {
                    continue;
                };
                let (label, handler) = match name.as_str() {
                    "addEventListener" => {
                        let Some(event) = arguments.iter().position(|a| matches!(strip(a), Expression::StringLiteral(_))) else {
                            continue;
                        };
                        let Expression::StringLiteral(literal) = strip(arguments[event]) else {
                            continue;
                        };
                        let Some(handler) = arguments.get(event + 1) else {
                            continue;
                        };
                        (literal.value.to_string(), *handler)
                    }
                    "requestAnimationFrame" | "setInterval" | "setTimeout" => {
                        let Some(handler) = arguments.first() else {
                            continue;
                        };
                        (name.clone(), *handler)
                    }
                    _ => continue,
                };
                let functions = self.handler_functions(handler, *start, func);
                if functions.iter().any(|span| touched(*span)) {
                    // Handlers outside the slot spans hold the record
                    // literals the slot's labels come from.
                    for span in &functions {
                        let span = SourceSpan {
                            start: span.start,
                            end: span.end,
                        };
                        if !handlers.contains(&span) {
                            handlers.push(span);
                        }
                    }
                    let read = facts.handler_members.entry(label.clone()).or_default();
                    for span in &functions {
                        let first = self.index.members.partition_point(|(start, _)| *start < span.start);
                        read.extend(
                            self.index.members[first..]
                                .iter()
                                .take_while(|(start, _)| *start < span.end)
                                .map(|(_, name)| name.clone()),
                        );
                    }
                    if name == "addEventListener" {
                        facts.events.insert(label);
                    } else {
                        facts.timers.insert(label);
                    }
                }
            }
        }
        handlers
    }

    /// Function spans a handler argument denotes, plus the program
    /// functions those call directly (a recorder behind a thin handler).
    fn handler_functions(&self, handler: &'a Expression<'a>, at: u32, func: Option<NodeId>) -> Vec<oxc_span::Span> {
        let nodes = self.parse.nodes;
        let mut out = Vec::new();
        let mut pending: Vec<(&'a Expression<'a>, u32, Option<NodeId>, usize)> = vec![(handler, at, func, 0)];
        let mut function_nodes: Vec<NodeId> = Vec::new();
        while let Some((expression, at, func, hop)) = pending.pop() {
            match strip(expression) {
                Expression::FunctionExpression(function) => {
                    out.push(function.span);
                    function_nodes.push(function.node_id.get());
                }
                Expression::ArrowFunctionExpression(arrow) => {
                    out.push(arrow.span);
                    function_nodes.push(arrow.node_id.get());
                }
                Expression::Identifier(_) if hop < 3 => {
                    let Some(symbol) = self.symbol_of(expression) else {
                        continue;
                    };
                    let declaration = self.parse.scoping.symbol_declaration(symbol);
                    if let AstKind::Function(function) = nodes.kind(declaration) {
                        out.push(function.span);
                        function_nodes.push(declaration);
                    }
                    for def in self.defs(symbol) {
                        if let Some(rhs) = def.rhs {
                            pending.push((rhs, def.start, def.func, hop + 1));
                        }
                    }
                    let _ = (at, func);
                }
                _ => {}
            }
        }
        // One level of direct calls to program functions.
        let mut called = Vec::new();
        for span in &out {
            let first = self.index.calls.partition_point(|(start, _)| *start < span.start);
            for (start, id) in &self.index.calls[first..] {
                if *start >= span.end {
                    break;
                }
                let AstKind::CallExpression(call) = nodes.kind(*id) else {
                    continue;
                };
                if let Some(symbol) = self.symbol_of(&call.callee) {
                    let declaration = self.parse.scoping.symbol_declaration(symbol);
                    if let AstKind::Function(function) = nodes.kind(declaration) {
                        called.push(function.span);
                    }
                }
            }
        }
        out.extend(called);
        out
    }

    /// Object literals of the slot spans keyed by any label the leaves read.
    fn literals(&mut self, facts: &mut SlotFacts) {
        let labels: BTreeSet<String> = facts.descs().flat_map(|desc| desc.labels.iter().cloned()).collect();
        if labels.is_empty() {
            return;
        }
        let nodes = self.parse.nodes;
        let mut out = Vec::new();
        let mut spans = self.spans.clone();
        for handler in &self.handlers {
            if !spans.iter().any(|span| handler.start >= span.start && handler.end <= span.end) {
                spans.push(*handler);
            }
        }
        for span in &spans {
            let first = self.index.objects.partition_point(|(start, _)| *start < span.start);
            let objects: Vec<(u32, NodeId)> = self.index.objects[first..]
                .iter()
                .take_while(|(start, _)| *start < span.end)
                .copied()
                .collect();
            for (_, id) in objects {
                let AstKind::ObjectExpression(object) = nodes.kind(id) else {
                    continue;
                };
                let mut keys: Vec<(String, &'a Expression<'a>, u32)> = object
                    .properties
                    .iter()
                    .filter_map(|property| match property {
                        ObjectPropertyKind::ObjectProperty(property) => property
                            .key
                            .static_name()
                            .map(|name| (name.to_string(), &property.value, object.span.start)),
                        _ => None,
                    })
                    .collect();
                keys.extend(self.member_assignments(id, object.span));
                if keys.len() < 2 || !keys.iter().any(|(key, _, _)| labels.contains(key)) {
                    continue;
                }
                let func = self.function_of(id);
                let mut map = BTreeMap::new();
                let mut held = BTreeSet::new();
                for (key, value, at) in keys {
                    if let Expression::Identifier(identifier) = strip(value) {
                        if let Some(symbol) = crate::analysis::identifier_symbol(identifier, self.parse.scoping) {
                            held.insert(symbol);
                        }
                    }
                    if let Expression::ArrayExpression(array) = strip(value) {
                        for (position, element) in array.elements.iter().enumerate() {
                            if let Some(element) = element.as_expression() {
                                let desc = self.describe(element, at, func);
                                map.insert(format!("{key}[{position}]"), desc);
                            }
                        }
                    }
                    let desc = self.describe(value, at, func);
                    map.insert(key, desc);
                }
                out.push(RecordLiteral {
                    fields: map,
                    containers: self.literal_containers(id, object.span),
                    held,
                });
            }
        }
        facts.literals = out;
    }

    /// `holder.k = v` statements completing an object literal held in a
    /// binding (`r = {a, b}; r.c = ...; r.d = ...`), up to the holder's
    /// next definition in the same function.
    fn member_assignments(&self, object: NodeId, span: oxc_span::Span) -> Vec<(String, &'a Expression<'a>, u32)> {
        let nodes = self.parse.nodes;
        let scoping = self.parse.scoping;
        let holder = match nodes.kind(nodes.parent_id(object)) {
            AstKind::VariableDeclarator(declarator) => {
                declarator.id.get_binding_identifier().and_then(|binding| binding.symbol_id.get())
            }
            AstKind::AssignmentExpression(assignment) => match &assignment.left {
                AssignmentTarget::AssignmentTargetIdentifier(identifier) => {
                    crate::analysis::identifier_symbol(identifier, scoping)
                }
                _ => None,
            },
            _ => None,
        };
        let Some(holder) = holder else {
            return Vec::new();
        };
        let func = self.function_of(object);
        let next_def = self
            .defs(holder)
            .iter()
            .filter(|def| def.func == func && def.start > span.end)
            .map(|def| def.start)
            .min()
            .unwrap_or(u32::MAX);
        let mut out = Vec::new();
        for reference in scoping.get_resolved_references(holder) {
            let node = reference.node_id();
            let at = nodes.get_node(node).span().start;
            if at < span.end || at >= next_def || self.function_of(node) != func {
                continue;
            }
            let member = nodes.parent_id(node);
            let AstKind::StaticMemberExpression(member_expression) = nodes.kind(member) else {
                continue;
            };
            let AstKind::AssignmentExpression(assignment) = nodes.kind(nodes.parent_id(member)) else {
                continue;
            };
            if matches!(&assignment.left, AssignmentTarget::StaticMemberExpression(left) if left.span == member_expression.span) {
                out.push((member_expression.property.name.to_string(), &assignment.right, assignment.span.start));
            }
        }
        out
    }
}

/// Lifecycle callbacks of a producer whose registrations were not proven
/// (an unrecovered coordinator): a function passed to a call of the
/// producer's formal parameter 2..=5 takes that position's role in the
/// producer callback ABI `(emitters, context, init, collection,
/// last_chance, cleanup)`. Requires the emitter array in formal 0 (read
/// as `p[k]`) and a registration through at least two registrar
/// positions, so a producer of another ABI yields nothing.
fn positional_callbacks(
    view: &SlotView,
    parse: &roles::FinderCtx<'_, '_>,
    index: &ProgramIndex,
) -> Vec<(LifecycleRole, SourceSpan)> {
    let Some(producer) = view.producer_span() else {
        return Vec::new();
    };
    let nodes = parse.nodes;
    let scoping = parse.scoping;
    // The producer function and its formals.
    let function = index
        .calls
        .iter()
        .skip_while(|(start, _)| *start < producer.start)
        .take_while(|(start, _)| *start < producer.end)
        .find_map(|(_, id)| {
            nodes.ancestor_ids(*id).find(|ancestor| {
                let span = nodes.kind(*ancestor).span();
                matches!(nodes.kind(*ancestor), AstKind::Function(_) | AstKind::ArrowFunctionExpression(_))
                    && span.start == producer.start
                    && span.end == producer.end
            })
        });
    let Some(function) = function else {
        return Vec::new();
    };
    let params = match nodes.kind(function) {
        AstKind::Function(function) => &function.params,
        AstKind::ArrowFunctionExpression(arrow) => &arrow.params,
        _ => return Vec::new(),
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
        return Vec::new();
    }
    let roles = [
        (2usize, LifecycleRole::Init),
        (3, LifecycleRole::Collection),
        (4, LifecycleRole::LastChance),
        (5, LifecycleRole::Cleanup),
    ];
    let mut out = Vec::new();
    for (position, role) in roles {
        let Some(Some(symbol)) = formals.get(position).copied() else {
            continue;
        };
        for reference in scoping.get_resolved_references(symbol) {
            let AstKind::CallExpression(call) = nodes.kind(nodes.parent_id(reference.node_id())) else {
                continue;
            };
            if !matches!(strip(&call.callee), Expression::Identifier(id) if id.span == nodes.get_node(reference.node_id()).span()) {
                continue;
            }
            for argument in call.arguments.iter().filter_map(Argument::as_expression) {
                let span = match strip(argument) {
                    Expression::FunctionExpression(function) => function.span,
                    Expression::ArrowFunctionExpression(arrow) => arrow.span,
                    _ => continue,
                };
                out.push((role, SourceSpan { start: span.start, end: span.end }));
            }
        }
    }
    let positions: BTreeSet<u8> = out
        .iter()
        .map(|(role, _)| match role {
            LifecycleRole::Init => 2,
            LifecycleRole::Collection => 3,
            LifecycleRole::LastChance => 4,
            _ => 5,
        })
        .collect();
    if positions.len() < 2 {
        return Vec::new();
    }
    out
}

/// `v ^ stream()` / `stream() ^ v` -> `v`.
fn unmask<'e, 'a>(expression: &'e Expression<'a>) -> &'e Expression<'a> {
    if let Expression::BinaryExpression(binary) = strip(expression) {
        if binary.operator.as_str() == "^" {
            let stream = |e: &Expression<'a>| matches!(strip(e), Expression::CallExpression(call) if call.arguments.is_empty());
            if stream(&binary.right) {
                return strip(&binary.left);
            }
            if stream(&binary.left) {
                return strip(&binary.right);
            }
        }
    }
    strip(expression)
}

/// Pick consecutive unused groups whose writes match the leaf ops.
fn match_groups(ops: &[LeafOp], groups: &[(SymbolId, Vec<&Write<'_>>)], used: &mut [bool]) -> Option<Vec<usize>> {
    let fits = |write: &Write<'_>, op: LeafOp| match write.kind {
        WriteKind::Store => op == LeafOp::Byte,
        WriteKind::Call => op != LeafOp::Byte,
        WriteKind::Site => false,
    };
    let loose = |_write: &Write<'_>, _op: LeafOp| true;
    for strict in [true, false] {
        for start in 0..groups.len() {
            if used[start] {
                continue;
            }
            let mut chosen = Vec::new();
            let mut cursor = 0usize;
            let mut index = start;
            while index < groups.len() && cursor < ops.len() && !used[index] {
                let group = &groups[index].1;
                if cursor + group.len() > ops.len() {
                    break;
                }
                let ok = group.iter().zip(&ops[cursor..]).all(|(write, op)| {
                    if strict {
                        fits(write, *op)
                    } else {
                        loose(write, *op)
                    }
                });
                if !ok {
                    break;
                }
                cursor += group.len();
                chosen.push(index);
                index += 1;
            }
            if cursor == ops.len() && !chosen.is_empty() {
                for index in &chosen {
                    used[*index] = true;
                }
                return Some(chosen);
            }
        }
    }
    None
}
