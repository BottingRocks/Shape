//! Flow-aware value resolution for packed flag bytes.
//!
//! [`FinderCtx::reaching`] gives, for one read of a local binding, every
//! store of that binding that can reach the read along the control flow of
//! the function holding it (branches, loops, `break` / `continue`, `try`,
//! `switch`), instead of the textually nearest one. Stores made in other
//! functions (closure state) reach a read that enters the function with the
//! binding's outer value.
//!
//! [`FinderCtx::sym`] turns an expression into a [`Sym`]: the expression
//! with every local read replaced by the values of its reaching stores
//! (a [`Sym::Phi`] when several reach), a `true` store replaced by the test
//! it is stored under ([`Sym::Guard`]), parameters and globals kept
//! symbolic. Packed bytes are split and their booleans named on this form.
//!
//! Like the catalog files, nothing here reads salts, slot addresses,
//! descriptor indices, generated spellings or build constants.

use std::collections::{BTreeSet, HashMap};

use oxc_ast::ast::{
    ArrowFunctionExpression, AssignmentExpression, AssignmentTarget, BinaryOperator, BreakStatement, Class,
    ConditionalExpression, ContinueStatement, DoWhileStatement, Expression, ForInStatement, ForOfStatement,
    ForStatement, ForStatementInit, ForStatementLeft, Function, IdentifierReference, IfStatement,
    LabeledStatement, LogicalExpression, LogicalOperator, ReturnStatement, SimpleAssignmentTarget, Statement,
    SwitchStatement, ThrowStatement, TryStatement, UnaryOperator, UpdateExpression, VariableDeclarator,
    WhileStatement,
};
use oxc_ast::AstKind;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{NodeId, ScopeFlags, SymbolId};
use oxc_span::{GetSpan, Span};

use crate::signals::analysis::FinderCtx;

// ===========================================================================
// Reaching stores
// ===========================================================================

/// One store of a binding.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Store {
    /// The value the binding has when the function starts: a parameter, a
    /// hoisted `var` (undefined), or the outer value of a captured binding.
    Entry,
    /// `x = v` (the assignment node).
    Assign(NodeId),
    /// `let x = v` / `let x` (the declarator node).
    Declare(NodeId),
    /// `x |= v`, `x += v`, `x++`, `x ||= v`, a `for (x in …)` head, a catch
    /// parameter: a value this resolver does not follow.
    Other(NodeId),
}

/// The reaching stores of every read of one binding in one function.
#[derive(Default)]
struct Reaching {
    reads: HashMap<NodeId, BTreeSet<Store>>,
    /// Stores the walk did not see (a destructuring target, …): no read of
    /// the binding is resolved.
    unsound: bool,
}

#[derive(Hash, PartialEq, Eq)]
struct ReachingKey(NodeId, SymbolId);

type State = Option<BTreeSet<Store>>;

fn join(a: State, b: State) -> State {
    match (a, b) {
        (None, b) => b,
        (a, None) => a,
        (Some(mut a), Some(b)) => {
            a.extend(b);
            Some(a)
        }
    }
}

struct Target {
    label: Option<String>,
    is_loop: bool,
    breakable: bool,
    breaks: State,
    continues: State,
}

/// Forward walk of one function body for one binding.
struct Flow<'r> {
    symbol: SymbolId,
    scoping: &'r oxc_semantic::Scoping,
    state: State,
    reads: HashMap<NodeId, BTreeSet<Store>>,
    stores: BTreeSet<NodeId>,
    targets: Vec<Target>,
    label: Option<String>,
    tries: Vec<BTreeSet<Store>>,
    /// Stores of nested functions (they may run at any time).
    nested: usize,
}

impl Flow<'_> {
    fn is_symbol(&self, identifier: &IdentifierReference<'_>) -> bool {
        crate::oxc::identifier_symbol(identifier, self.scoping) == Some(self.symbol)
    }

    fn store(&mut self, store: Store) {
        if let Store::Assign(node) | Store::Declare(node) | Store::Other(node) = &store {
            self.stores.insert(*node);
        }
        for accumulator in &mut self.tries {
            accumulator.insert(store.clone());
        }
        if self.state.is_some() {
            self.state = Some(BTreeSet::from([store]));
        }
    }

    fn read(&mut self, node: NodeId) {
        if let Some(state) = &self.state {
            self.reads.entry(node).or_default().extend(state.iter().cloned());
        }
    }

    fn jump_break(&mut self, label: Option<&str>) {
        let state = self.state.take();
        let target = self.targets.iter_mut().rev().find(|target| match label {
            Some(label) => target.label.as_deref() == Some(label),
            None => target.breakable,
        });
        if let Some(target) = target {
            target.breaks = join(target.breaks.take(), state);
        }
    }

    fn jump_continue(&mut self, label: Option<&str>) {
        let state = self.state.take();
        let target = self.targets.iter_mut().rev().find(|target| {
            target.is_loop && label.is_none_or(|label| target.label.as_deref() == Some(label))
        });
        if let Some(target) = target {
            target.continues = join(target.continues.take(), state);
        }
    }

    /// Run a loop to a fixpoint. `head` evaluates the test (returns the
    /// state leaving the loop when the test fails, `None` without a test);
    /// `body` the body and the update.
    fn looped(
        &mut self,
        label: Option<String>,
        test_first: bool,
        mut head: impl FnMut(&mut Self) -> State,
        mut body: impl FnMut(&mut Self),
    ) {
        let entry = self.state.clone();
        let mut at_head = entry.clone();
        let mut exit: State = None;
        for _ in 0..8 {
            self.state = at_head.clone();
            let mut leave = None;
            if test_first {
                leave = head(self);
            }
            self.targets.push(Target {
                label: label.clone(),
                is_loop: true,
                breakable: true,
                breaks: None,
                continues: None,
            });
            body(self);
            let target = self.targets.pop().expect("loop target");
            self.state = join(self.state.take(), target.continues);
            if !test_first {
                leave = head(self);
            }
            exit = join(leave, target.breaks);
            let next = join(entry.clone(), self.state.take());
            if next == at_head {
                break;
            }
            at_head = next;
        }
        self.state = exit;
    }
}

impl<'a> Visit<'a> for Flow<'_> {
    fn visit_function(&mut self, it: &Function<'a>, _flags: ScopeFlags) {
        self.nested_stores(it.span);
    }

    fn visit_arrow_function_expression(&mut self, it: &ArrowFunctionExpression<'a>) {
        self.nested_stores(it.span);
    }

    fn visit_class(&mut self, it: &Class<'a>) {
        self.nested_stores(it.span);
    }

    fn visit_identifier_reference(&mut self, it: &IdentifierReference<'a>) {
        if self.is_symbol(it) {
            self.read(it.node_id.get());
        }
    }

    fn visit_variable_declarator(&mut self, it: &VariableDeclarator<'a>) {
        if let Some(init) = &it.init {
            self.visit_expression(init);
        }
        let declares = match &it.id {
            oxc_ast::ast::BindingPattern::BindingIdentifier(identifier) => identifier.symbol_id.get() == Some(self.symbol),
            _ => false,
        };
        if declares {
            // `var x;` keeps the value; `let x;` is undefined.
            let keeps = it.init.is_none() && it.kind == oxc_ast::ast::VariableDeclarationKind::Var;
            if !keeps {
                self.store(Store::Declare(it.node_id.get()));
            }
        } else {
            self.visit_binding_pattern(&it.id);
        }
    }

    fn visit_assignment_expression(&mut self, it: &AssignmentExpression<'a>) {
        let node = it.node_id.get();
        match &it.left {
            AssignmentTarget::AssignmentTargetIdentifier(identifier) if self.is_symbol(identifier) => {
                if it.operator.is_assign() {
                    self.visit_expression(&it.right);
                    self.store(Store::Assign(node));
                } else if it.operator.is_logical() {
                    // `x ||= v`: the old value or the new one.
                    self.read(identifier.node_id.get());
                    let before = self.state.clone();
                    self.visit_expression(&it.right);
                    self.store(Store::Other(node));
                    self.state = join(self.state.take(), before);
                } else {
                    self.read(identifier.node_id.get());
                    self.visit_expression(&it.right);
                    self.store(Store::Other(node));
                }
            }
            left => {
                self.visit_assignment_target(left);
                self.visit_expression(&it.right);
            }
        }
    }

    fn visit_update_expression(&mut self, it: &UpdateExpression<'a>) {
        match &it.argument {
            SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier) if self.is_symbol(identifier) => {
                self.read(identifier.node_id.get());
                self.store(Store::Other(it.node_id.get()));
            }
            _ => walk::walk_update_expression(self, it),
        }
    }

    fn visit_conditional_expression(&mut self, it: &ConditionalExpression<'a>) {
        self.visit_expression(&it.test);
        let after_test = self.state.clone();
        self.visit_expression(&it.consequent);
        let consequent = self.state.take();
        self.state = after_test;
        self.visit_expression(&it.alternate);
        self.state = join(consequent, self.state.take());
    }

    fn visit_logical_expression(&mut self, it: &LogicalExpression<'a>) {
        self.visit_expression(&it.left);
        let after_left = self.state.clone();
        self.visit_expression(&it.right);
        self.state = join(after_left, self.state.take());
    }

    fn visit_if_statement(&mut self, it: &IfStatement<'a>) {
        self.visit_expression(&it.test);
        let after_test = self.state.clone();
        self.visit_statement(&it.consequent);
        let consequent = self.state.take();
        self.state = after_test;
        if let Some(alternate) = &it.alternate {
            self.visit_statement(alternate);
        }
        self.state = join(consequent, self.state.take());
    }

    fn visit_labeled_statement(&mut self, it: &LabeledStatement<'a>) {
        let label = it.label.name.to_string();
        let is_loop = matches!(
            it.body,
            Statement::ForStatement(_)
                | Statement::WhileStatement(_)
                | Statement::DoWhileStatement(_)
                | Statement::ForInStatement(_)
                | Statement::ForOfStatement(_)
        );
        if is_loop {
            self.label = Some(label);
            self.visit_statement(&it.body);
            return;
        }
        self.targets.push(Target {
            label: Some(label),
            is_loop: false,
            breakable: false,
            breaks: None,
            continues: None,
        });
        self.visit_statement(&it.body);
        let target = self.targets.pop().expect("label target");
        self.state = join(self.state.take(), target.breaks);
    }

    fn visit_break_statement(&mut self, it: &BreakStatement<'a>) {
        let label = it.label.as_ref().map(|label| label.name.to_string());
        self.jump_break(label.as_deref());
    }

    fn visit_continue_statement(&mut self, it: &ContinueStatement<'a>) {
        let label = it.label.as_ref().map(|label| label.name.to_string());
        self.jump_continue(label.as_deref());
    }

    fn visit_return_statement(&mut self, it: &ReturnStatement<'a>) {
        if let Some(argument) = &it.argument {
            self.visit_expression(argument);
        }
        self.state = None;
    }

    fn visit_throw_statement(&mut self, it: &ThrowStatement<'a>) {
        self.visit_expression(&it.argument);
        self.state = None;
    }

    fn visit_while_statement(&mut self, it: &WhileStatement<'a>) {
        let label = self.label.take();
        self.looped(
            label,
            true,
            |flow| {
                flow.visit_expression(&it.test);
                flow.state.clone()
            },
            |flow| flow.visit_statement(&it.body),
        );
    }

    fn visit_do_while_statement(&mut self, it: &DoWhileStatement<'a>) {
        let label = self.label.take();
        self.looped(
            label,
            false,
            |flow| {
                flow.visit_expression(&it.test);
                flow.state.clone()
            },
            |flow| flow.visit_statement(&it.body),
        );
    }

    fn visit_for_statement(&mut self, it: &ForStatement<'a>) {
        let label = self.label.take();
        if let Some(init) = &it.init {
            match init {
                ForStatementInit::VariableDeclaration(declaration) => self.visit_variable_declaration(declaration),
                other => {
                    if let Some(expression) = other.as_expression() {
                        self.visit_expression(expression);
                    }
                }
            }
        }
        self.looped(
            label,
            true,
            |flow| match &it.test {
                Some(test) => {
                    flow.visit_expression(test);
                    flow.state.clone()
                }
                None => None,
            },
            |flow| {
                flow.visit_statement(&it.body);
                // `continue` joins before the update.
                if let Some(target) = flow.targets.last_mut() {
                    let continues = target.continues.take();
                    flow.state = join(flow.state.take(), continues);
                }
                if let Some(update) = &it.update {
                    flow.visit_expression(update);
                }
            },
        );
    }

    fn visit_for_in_statement(&mut self, it: &ForInStatement<'a>) {
        let label = self.label.take();
        self.visit_expression(&it.right);
        self.each_loop(label, &it.left, &it.body);
    }

    fn visit_for_of_statement(&mut self, it: &ForOfStatement<'a>) {
        let label = self.label.take();
        self.visit_expression(&it.right);
        self.each_loop(label, &it.left, &it.body);
    }

    fn visit_switch_statement(&mut self, it: &SwitchStatement<'a>) {
        self.visit_expression(&it.discriminant);
        let entry = self.state.clone();
        self.targets.push(Target {
            label: self.label.take(),
            is_loop: false,
            breakable: true,
            breaks: None,
            continues: None,
        });
        let mut fall: State = None;
        let mut has_default = false;
        for case in &it.cases {
            self.state = join(entry.clone(), fall.take());
            match &case.test {
                Some(test) => self.visit_expression(test),
                None => has_default = true,
            }
            self.visit_statements(&case.consequent);
            fall = self.state.take();
        }
        let target = self.targets.pop().expect("switch target");
        let mut exit = join(fall, target.breaks);
        if !has_default {
            exit = join(exit, entry);
        }
        self.state = exit;
    }

    fn visit_try_statement(&mut self, it: &TryStatement<'a>) {
        let entry = self.state.clone();
        self.tries.push(BTreeSet::new());
        self.visit_block_statement(&it.block);
        let inside = self.tries.pop().unwrap_or_default();
        let after_try = self.state.take();
        let catch_entry = join(entry, Some(inside));
        let mut out = after_try;
        if let Some(handler) = &it.handler {
            self.state = catch_entry.clone();
            if let Some(parameter) = &handler.param {
                self.visit_binding_pattern(&parameter.pattern);
            }
            self.visit_block_statement(&handler.body);
            out = join(out, self.state.take());
        }
        if let Some(finalizer) = &it.finalizer {
            let reachable = out.is_some();
            self.state = join(out, catch_entry);
            self.visit_block_statement(finalizer);
            if !reachable {
                self.state = None;
            }
        } else {
            self.state = out;
        }
    }

    fn visit_binding_identifier(&mut self, it: &oxc_ast::ast::BindingIdentifier<'a>) {
        // A catch parameter or other binding of the symbol outside a
        // declarator: a value not followed.
        if it.symbol_id.get() == Some(self.symbol) {
            self.store(Store::Other(it.node_id.get()));
        }
    }
}

impl Flow<'_> {
    fn each_loop<'a>(&mut self, label: Option<String>, left: &ForStatementLeft<'a>, body: &Statement<'a>) {
        let left_store = match left {
            ForStatementLeft::AssignmentTargetIdentifier(identifier) if self.is_symbol(identifier) => {
                Some(identifier.node_id.get())
            }
            _ => None,
        };
        self.looped(
            label,
            true,
            |flow| flow.state.clone(),
            |flow| {
                match left_store {
                    Some(node) => flow.store(Store::Other(node)),
                    None => flow.visit_for_statement_left(left),
                }
                flow.visit_statement(body);
            },
        );
    }

    fn nested_stores(&mut self, _span: Span) {
        self.nested += 1;
    }
}

impl FinderCtx<'_, '_, '_> {
    /// The innermost function (or the program) holding `node`.
    pub(crate) fn flow_root(&self, node: NodeId) -> NodeId {
        let nodes = self.parse.nodes;
        let mut child = node;
        loop {
            let parent = nodes.parent_id(child);
            if parent == child {
                return child;
            }
            if matches!(nodes.kind(parent), AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)) {
                return parent;
            }
            child = parent;
        }
    }

    fn inside(&self, node: NodeId, root: NodeId) -> bool {
        self.flow_root(node) == root
    }

    fn reaching_table(&self, root: NodeId, symbol: SymbolId) -> std::rc::Rc<Reaching> {
        self.memo(ReachingKey(root, symbol), || {
            let scoping = self.parse.scoping;
            let mut flow = Flow {
                symbol,
                scoping,
                state: Some(BTreeSet::from([Store::Entry])),
                reads: HashMap::new(),
                stores: BTreeSet::new(),
                targets: Vec::new(),
                label: None,
                tries: Vec::new(),
                nested: 0,
            };
            match self.parse.nodes.kind(root) {
                AstKind::Function(function) => {
                    if let Some(body) = &function.body {
                        flow.visit_function_body(body);
                    }
                }
                AstKind::ArrowFunctionExpression(arrow) => flow.visit_function_body(&arrow.body),
                AstKind::Program(program) => flow.visit_statements(&program.body),
                _ => return Reaching { unsound: true, ..Reaching::default() },
            }
            // Every write of the binding inside this function must be a
            // store the walk saw.
            let mut unsound = false;
            for reference in scoping.get_resolved_references(symbol) {
                if !reference.is_write() {
                    continue;
                }
                let node = reference.node_id();
                if !self.inside(node, root) {
                    continue;
                }
                let seen = flow.stores.iter().any(|store| {
                    let span = self.parse.nodes.kind(*store).span();
                    let write = self.parse.nodes.kind(node).span();
                    span.start <= write.start && write.end <= span.end
                });
                if !seen {
                    unsound = true;
                }
            }
            Reaching { reads: flow.reads, unsound }
        })
    }

    /// The stores of the binding read by the identifier node `read` that
    /// can reach it. `None`: not a local binding read, or a store the walk
    /// cannot place.
    pub(crate) fn reaching(&self, read: NodeId) -> Option<(SymbolId, BTreeSet<Store>)> {
        let symbol = self.reference_symbol(read)?;
        let root = self.flow_root(read);
        let table = self.reaching_table(root, symbol);
        if table.unsound {
            return None;
        }
        let mut stores = table.reads.get(&read).cloned().unwrap_or_default();
        // Stores inside functions nested in this one may run between any
        // two statements.
        for reference in self.parse.scoping.get_resolved_references(symbol) {
            if !reference.is_write() {
                continue;
            }
            let node = reference.node_id();
            let inner = self.flow_root(node);
            if inner != root && self.within_function(inner, root) {
                stores.insert(self.store_of_write(node));
            }
        }
        Some((symbol, stores))
    }

    /// `inner` is a function nested (at any depth) in `outer`.
    fn within_function(&self, inner: NodeId, outer: NodeId) -> bool {
        let outer_span = self.parse.nodes.kind(outer).span();
        let inner_span = self.parse.nodes.kind(inner).span();
        inner != outer && outer_span.start <= inner_span.start && inner_span.end <= outer_span.end
    }

    /// The store node of a write reference.
    fn store_of_write(&self, node: NodeId) -> Store {
        let nodes = self.parse.nodes;
        let parent = nodes.parent_id(node);
        match nodes.kind(parent) {
            AstKind::AssignmentExpression(assignment) if assignment.operator.is_assign() => Store::Assign(parent),
            _ => Store::Other(parent),
        }
    }

    /// The stores of a binding outside the function `root` (the values a
    /// captured binding may hold when `root` runs).
    pub(crate) fn outer_stores(&self, symbol: SymbolId, root: NodeId) -> Vec<Store> {
        let nodes = self.parse.nodes;
        let scoping = self.parse.scoping;
        let mut out = Vec::new();
        let declaration = scoping.symbol_declaration(symbol);
        if let AstKind::VariableDeclarator(declarator) = nodes.kind(declaration) {
            if declarator.init.is_some() {
                out.push(Store::Declare(declaration));
            }
        }
        for reference in scoping.get_resolved_references(symbol) {
            if !reference.is_write() {
                continue;
            }
            let node = reference.node_id();
            let at = self.flow_root(node);
            if at == root || self.within_function(at, root) {
                continue;
            }
            out.push(self.store_of_write(node));
        }
        out
    }
}

// ===========================================================================
// Symbolic values
// ===========================================================================

/// A resolved value.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Sym {
    Num(f64),
    Str(String),
    Bool(bool),
    Null,
    Undef,
    /// An unbound global read.
    Global(String),
    /// Formal `index` of the function node.
    Param(NodeId, usize),
    Member(Box<Sym>, String),
    Index(Box<Sym>, Box<Sym>),
    Call(Box<Sym>, Vec<Sym>),
    New(Box<Sym>, Vec<Sym>),
    Unary(&'static str, Box<Sym>),
    Bin(&'static str, Box<Sym>, Box<Sym>),
    Cond(Box<Sym>, Box<Sym>, Box<Sym>),
    /// `true` stored under a test (`true`: in the consequent / right of
    /// `&&`; `false`: alternate / right of `||`).
    Guard(Box<Sym>, bool),
    Phi(Vec<Sym>),
    Array(Vec<Sym>),
    Object(Vec<(String, Sym)>),
    /// A function literal.
    Func(NodeId),
    /// Element `k` of a local binding (`state[k]`).
    Slot(SymbolId, u32),
    /// A read of a local binding and its value ([`FinderCtx::sym_with`]).
    Read(SymbolId, Box<Sym>),
    Regex(String),
    /// A value not followed (the reason).
    Opaque(&'static str),
}

/// Identifier hops followed while resolving one value.
const SYM_HOPS: usize = 24;
/// Sym nodes built for one value.
const SYM_SIZE: usize = 4000;

struct Resolver {
    hops: usize,
    size: usize,
    /// Stores being resolved (cycles).
    active: Vec<Store>,
    /// Keep each local read as [`Sym::Read`].
    reads: bool,
    /// Binding reads already resolved in full (no hop, cycle or size cut
    /// inside): the same value wherever the read is reached from, so a
    /// read met again (a boolean packed after `x && !!r && r.k` copies)
    /// costs no more of the budget.
    resolved: HashMap<NodeId, Sym>,
    /// Captured bindings' outer values already resolved in full, by
    /// binding and reading function (the same for every read there).
    captured: HashMap<(SymbolId, NodeId), Sym>,
}

impl Sym {
    pub(crate) fn render(&self) -> String {
        match self {
            Sym::Num(value) => format!("{value}"),
            Sym::Str(text) => format!("{text:?}"),
            Sym::Bool(value) => format!("{value}"),
            Sym::Null => "null".into(),
            Sym::Undef => "undefined".into(),
            Sym::Global(name) => name.clone(),
            Sym::Param(_, index) => format!("P{index}"),
            Sym::Member(object, name) => format!("{}.{name}", object.render()),
            Sym::Index(object, index) => format!("{}[{}]", object.render(), index.render()),
            Sym::Call(callee, args) => format!(
                "{}({})",
                callee.render(),
                args.iter().map(Sym::render).collect::<Vec<_>>().join(", ")
            ),
            Sym::New(callee, args) => format!(
                "new {}({})",
                callee.render(),
                args.iter().map(Sym::render).collect::<Vec<_>>().join(", ")
            ),
            Sym::Unary(op, value) => format!("{op}({})", value.render()),
            Sym::Bin(op, left, right) => format!("({} {op} {})", left.render(), right.render()),
            Sym::Cond(test, consequent, alternate) => {
                format!("({} ? {} : {})", test.render(), consequent.render(), alternate.render())
            }
            Sym::Guard(test, polarity) => format!("guard{}({})", if *polarity { "" } else { "!" }, test.render()),
            Sym::Phi(items) => format!("phi({})", items.iter().map(Sym::render).collect::<Vec<_>>().join(" | ")),
            Sym::Array(items) => format!("[{}]", items.iter().map(Sym::render).collect::<Vec<_>>().join(", ")),
            Sym::Object(entries) => format!(
                "{{{}}}",
                entries
                    .iter()
                    .map(|(key, value)| format!("{key}: {}", value.render()))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Sym::Func(_) => "fn".into(),
            Sym::Slot(_, index) => format!("slot[{index}]"),
            Sym::Read(_, value) => format!("read<{}>", value.render()),
            Sym::Regex(pattern) => format!("/{pattern}/"),
            Sym::Opaque(reason) => format!("?{reason}"),
        }
    }

    /// Every sub-value (pre-order, this one first).
    pub(crate) fn walk<'s>(&'s self, out: &mut Vec<&'s Sym>) {
        out.push(self);
        match self {
            Sym::Member(object, _) => object.walk(out),
            Sym::Index(object, index) => {
                object.walk(out);
                index.walk(out);
            }
            Sym::Call(callee, args) | Sym::New(callee, args) => {
                callee.walk(out);
                for arg in args {
                    arg.walk(out);
                }
            }
            Sym::Unary(_, value) | Sym::Guard(value, _) | Sym::Read(_, value) => value.walk(out),
            Sym::Bin(_, left, right) => {
                left.walk(out);
                right.walk(out);
            }
            Sym::Cond(test, consequent, alternate) => {
                test.walk(out);
                consequent.walk(out);
                alternate.walk(out);
            }
            Sym::Phi(items) | Sym::Array(items) => {
                for item in items {
                    item.walk(out);
                }
            }
            Sym::Object(entries) => {
                for (_, value) in entries {
                    value.walk(out);
                }
            }
            _ => {}
        }
    }

    /// A hop, cycle or size cut inside: the value depends on where the
    /// resolution started.
    fn has_cut(&self) -> bool {
        self.nodes()
            .iter()
            .any(|node| matches!(node, Sym::Opaque("hops") | Sym::Opaque("cycle") | Sym::Opaque("size")))
    }

    pub(crate) fn nodes(&self) -> Vec<&Sym> {
        let mut out = Vec::new();
        self.walk(&mut out);
        out
    }

    /// A value that carries no truth of its own (`false`, `0`, `undefined`,
    /// `null`): a flag stored as this is "not set".
    pub(crate) fn is_unset(&self) -> bool {
        matches!(self, Sym::Bool(false) | Sym::Undef | Sym::Null) || matches!(self, Sym::Num(value) if *value == 0.0)
    }

    /// The alternatives of a phi (nested phis flattened), unset ones
    /// dropped.
    pub(crate) fn alternatives(&self) -> Vec<&Sym> {
        let mut out = Vec::new();
        fn flatten<'s>(sym: &'s Sym, out: &mut Vec<&'s Sym>) {
            match sym {
                Sym::Phi(items) => items.iter().for_each(|item| flatten(item, out)),
                other if other.is_unset() => {}
                other => out.push(other),
            }
        }
        flatten(self, &mut out);
        out
    }

    fn phi(items: Vec<Sym>) -> Sym {
        let mut out: Vec<Sym> = Vec::new();
        for item in items {
            let parts = match item {
                Sym::Phi(inner) => inner,
                other => vec![other],
            };
            for part in parts {
                if !out.contains(&part) {
                    out.push(part);
                }
            }
        }
        if out.len() == 1 {
            out.pop().expect("one")
        } else {
            Sym::Phi(out)
        }
    }

    /// Strings of the value (literals at any depth).
    pub(crate) fn strings(&self) -> Vec<&str> {
        self.nodes()
            .into_iter()
            .filter_map(|node| match node {
                Sym::Str(text) => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }

    /// Member names read at any depth.
    pub(crate) fn members(&self) -> Vec<&str> {
        self.nodes()
            .into_iter()
            .filter_map(|node| match node {
                Sym::Member(_, name) => Some(name.as_str()),
                _ => None,
            })
            .collect()
    }

    pub(crate) fn has_member(&self, name: &str) -> bool {
        self.members().contains(&name)
    }

    pub(crate) fn has_string(&self, text: &str) -> bool {
        self.strings().contains(&text)
    }

    pub(crate) fn has_global(&self, name: &str) -> bool {
        self.nodes().iter().any(|node| matches!(node, Sym::Global(global) if global == name))
    }
}

impl FinderCtx<'_, '_, '_> {
    /// The resolved value of the expression `node`.
    pub(crate) fn sym(&self, node: NodeId) -> Sym {
        self.sym_with(node, false)
    }

    /// [`Self::sym`], each local read kept as [`Sym::Read`] (`reads`).
    pub(crate) fn sym_with(&self, node: NodeId, reads: bool) -> Sym {
        let mut resolver = Resolver {
            hops: SYM_HOPS,
            size: 0,
            active: Vec::new(),
            reads,
            resolved: HashMap::new(),
            captured: HashMap::new(),
        };
        self.sym_in(node, &mut resolver)
    }

    fn sym_expression(&self, expression: &Expression<'_>, resolver: &mut Resolver) -> Sym {
        match self.node_with_span(expression.without_parentheses().span()) {
            Some(node) => self.sym_in(node, resolver),
            None => Sym::Opaque("node"),
        }
    }

    fn sym_in(&self, node: NodeId, resolver: &mut Resolver) -> Sym {
        let nodes = self.parse.nodes;
        // A literal costs nothing to read: the size cut never hides one
        // (the `n` of `x << n` after a large `x`).
        match nodes.kind(node) {
            AstKind::NumericLiteral(literal) => return Sym::Num(literal.value),
            AstKind::BooleanLiteral(literal) => return Sym::Bool(literal.value),
            AstKind::NullLiteral(_) => return Sym::Null,
            _ => {}
        }
        resolver.size += 1;
        if resolver.size > SYM_SIZE {
            return Sym::Opaque("size");
        }
        match nodes.kind(node) {
            AstKind::ParenthesizedExpression(inner) => self.sym_expression(&inner.expression, resolver),
            AstKind::NumericLiteral(literal) => Sym::Num(literal.value),
            AstKind::StringLiteral(literal) => Sym::Str(literal.value.to_string()),
            AstKind::BooleanLiteral(literal) => Sym::Bool(literal.value),
            AstKind::NullLiteral(_) => Sym::Null,
            AstKind::RegExpLiteral(literal) => Sym::Regex(literal.regex.pattern.text.to_string()),
            AstKind::TemplateLiteral(template) if template.expressions.is_empty() => Sym::Str(
                template
                    .quasis
                    .iter()
                    .map(|quasi| quasi.value.cooked.as_ref().map_or("", |cooked| cooked.as_str()))
                    .collect(),
            ),
            AstKind::IdentifierReference(identifier) => self.sym_identifier(node, identifier, resolver),
            AstKind::StaticMemberExpression(member) => {
                if let Some(value) = self.sealed_literal(&member.object, member.property.name.as_str()) {
                    return value;
                }
                Sym::Member(Box::new(self.sym_expression(&member.object, resolver)), member.property.name.to_string())
            }
            AstKind::ComputedMemberExpression(member) => {
                // `t[k]` of the tracer's argument tuple (`t = [i, v, m];
                // buf[t[0]] = t[1] ^ t[2]()`): the element itself.
                if let Some(element) = self.tuple_member_slot(member) {
                    return self.sym_in(element, resolver);
                }
                // `state[k]` of a local binding: an element slot whose
                // stores are elsewhere (not the initial literal).
                if let (Expression::Identifier(identifier), Expression::NumericLiteral(index), false) =
                    (member.object.without_parentheses(), member.expression.without_parentheses(), resolver.reads)
                {
                    if let Some(symbol) = self.identifier_symbol(identifier) {
                        if index.value.fract() == 0.0 && index.value >= 0.0 {
                            return Sym::Slot(symbol, index.value as u32);
                        }
                    }
                }
                let object = self.sym_expression(&member.object, resolver);
                match self.sym_expression(&member.expression, resolver) {
                    Sym::Str(name) => Sym::Member(Box::new(object), name),
                    index => Sym::Index(Box::new(object), Box::new(index)),
                }
            }
            AstKind::CallExpression(call) => {
                let callee = self.sym_expression(&call.callee, resolver);
                let args: Vec<Sym> = call
                    .arguments
                    .iter()
                    .map(|argument| match argument.as_expression() {
                        Some(expression) => self.sym_expression(expression, resolver),
                        None => Sym::Opaque("spread"),
                    })
                    .collect();
                // A helper that returns one of its formals unchanged
                // (`function h(a, b, c, d, e) { b[c].call(a, d); return e; }`,
                // a VM subroutine carrying a register through): the call's
                // value is that argument.
                if let Sym::Func(function) = peel_reads(&callee) {
                    if let Some(index) = self.returned_formal(*function) {
                        return args.get(index).cloned().unwrap_or(Sym::Undef);
                    }
                }
                fold_call(callee, args)
            }
            AstKind::NewExpression(new) => {
                let callee = self.sym_expression(&new.callee, resolver);
                let args = new
                    .arguments
                    .iter()
                    .map(|argument| match argument.as_expression() {
                        Some(expression) => self.sym_expression(expression, resolver),
                        None => Sym::Opaque("spread"),
                    })
                    .collect();
                Sym::New(Box::new(callee), args)
            }
            AstKind::UnaryExpression(unary) => {
                let value = self.sym_expression(&unary.argument, resolver);
                match (unary.operator, &value) {
                    (UnaryOperator::Void, _) => Sym::Undef,
                    (UnaryOperator::LogicalNot, Sym::Num(number)) => Sym::Bool(*number == 0.0),
                    (UnaryOperator::LogicalNot, Sym::Bool(value)) => Sym::Bool(!value),
                    (UnaryOperator::UnaryNegation, Sym::Num(number)) => Sym::Num(-number),
                    _ => Sym::Unary(unary.operator.as_str(), Box::new(value)),
                }
            }
            AstKind::BinaryExpression(binary) => {
                let left = self.sym_expression(&binary.left, resolver);
                let right = self.sym_expression(&binary.right, resolver);
                Sym::Bin(binary.operator.as_str(), Box::new(left), Box::new(right))
            }
            AstKind::LogicalExpression(logical) => {
                let left = self.sym_expression(&logical.left, resolver);
                let right = self.sym_expression(&logical.right, resolver);
                Sym::Bin(logical.operator.as_str(), Box::new(left), Box::new(right))
            }
            AstKind::ConditionalExpression(conditional) => {
                let test = self.sym_expression(&conditional.test, resolver);
                let consequent = self.sym_expression(&conditional.consequent, resolver);
                let alternate = self.sym_expression(&conditional.alternate, resolver);
                Sym::Cond(Box::new(test), Box::new(consequent), Box::new(alternate))
            }
            AstKind::SequenceExpression(sequence) => match sequence.expressions.last() {
                Some(last) => self.sym_expression(last, resolver),
                None => Sym::Undef,
            },
            AstKind::AssignmentExpression(assignment) if assignment.operator.is_assign() => {
                self.sym_expression(&assignment.right, resolver)
            }
            AstKind::ArrayExpression(array) => Sym::Array(
                array
                    .elements
                    .iter()
                    .map(|element| match element.as_expression() {
                        Some(expression) => self.sym_expression(expression, resolver),
                        None => Sym::Opaque("hole"),
                    })
                    .collect(),
            ),
            AstKind::ObjectExpression(object) => Sym::Object(
                object
                    .properties
                    .iter()
                    .filter_map(|property| match property {
                        oxc_ast::ast::ObjectPropertyKind::ObjectProperty(property) => {
                            let key = property.key.static_name()?.to_string();
                            Some((key, self.sym_expression(&property.value, resolver)))
                        }
                        _ => None,
                    })
                    .collect(),
            ),
            AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) => Sym::Func(node),
            _ => Sym::Opaque("expression"),
        }
    }

    /// `o.k` where every store of `o` reaching the read is an object
    /// literal holding the same literal primitive under `k`, and none of
    /// those objects can have `k` changed: wherever a binding may hold one
    /// (`o`, or a binding it is copied into), the read only reads a named
    /// member of it (no argument, method call, computed member or other
    /// escape) and never writes `k`. The stored literal (a made object's
    /// field no code can change).
    fn sealed_literal(&self, object: &Expression<'_>, key: &str) -> Option<Sym> {
        use oxc_ast::ast::{ObjectPropertyKind, PropertyKind};
        let Expression::Identifier(_) = object.without_parentheses() else { return None };
        let nodes = self.parse.nodes;
        let read = self.node_with_span(object.without_parentheses().span())?;
        let (symbol, stores) = self.reaching(read)?;
        let mut value: Option<Sym> = None;
        let mut made = BTreeSet::new();
        for store in &stores {
            let (Store::Assign(node) | Store::Declare(node)) = store else { return None };
            if let AstKind::AssignmentExpression(assignment) = nodes.kind(*node) {
                if !assignment.operator.is_assign() {
                    return None;
                }
            }
            let AstKind::ObjectExpression(literal) = nodes.kind(self.definition_value(*node)?) else { return None };
            let mut found = None;
            for property in &literal.properties {
                let ObjectPropertyKind::ObjectProperty(property) = property else { return None };
                let name = property.key.static_name()?;
                if name == "__proto__" {
                    return None;
                }
                if name == key {
                    if found.is_some() || property.kind != PropertyKind::Init {
                        return None;
                    }
                    found = Some(&property.value);
                }
            }
            let stored = match found?.without_parentheses() {
                Expression::BooleanLiteral(literal) => Sym::Bool(literal.value),
                Expression::NumericLiteral(literal) => Sym::Num(literal.value),
                Expression::StringLiteral(literal) => Sym::Str(literal.value.to_string()),
                Expression::NullLiteral(_) => Sym::Null,
                _ => return None,
            };
            if value.as_ref().is_some_and(|known| *known != stored) {
                return None;
            }
            value = Some(stored);
            made.insert(*node);
        }
        // Holders: a binding and the stores that put one of the objects in
        // it.
        let mut holders: Vec<(SymbolId, BTreeSet<NodeId>)> = vec![(symbol, made)];
        let mut checked = 0;
        while let Some((holder, puts)) = holders.pop() {
            checked += 1;
            if checked > 16 {
                return None;
            }
            for reference in self.parse.scoping.get_resolved_references(holder) {
                if !reference.is_read() {
                    continue;
                }
                let use_node = reference.node_id();
                let may_hold = match self.reaching(use_node) {
                    Some((_, stores)) => stores.iter().any(|store| match store {
                        Store::Entry => true,
                        Store::Assign(node) | Store::Declare(node) | Store::Other(node) => puts.contains(node),
                    }),
                    None => true,
                };
                if !may_hold {
                    continue;
                }
                if reference.is_write() {
                    return None;
                }
                let use_span = nodes.kind(use_node).span();
                let parent = nodes.parent_id(use_node);
                let name = match nodes.kind(parent) {
                    AstKind::StaticMemberExpression(it) if it.object.span() == use_span => it.property.name.to_string(),
                    AstKind::ComputedMemberExpression(it) if it.object.span() == use_span => {
                        match it.expression.without_parentheses() {
                            Expression::StringLiteral(text) => text.value.to_string(),
                            _ => return None,
                        }
                    }
                    // A copy into another binding: that binding holds it too.
                    AstKind::AssignmentExpression(assignment)
                        if assignment.operator.is_assign() && assignment.right.span() == use_span =>
                    {
                        let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else { return None };
                        let copy = self.identifier_symbol(target)?;
                        holders.push((copy, BTreeSet::from([parent])));
                        continue;
                    }
                    AstKind::VariableDeclarator(declarator)
                        if declarator.init.as_ref().is_some_and(|init| init.span() == use_span) =>
                    {
                        let copy = declarator.id.get_binding_identifier()?.symbol_id();
                        holders.push((copy, BTreeSet::from([parent])));
                        continue;
                    }
                    _ => return None,
                };
                let member_span = nodes.kind(parent).span();
                // A method call passes the object as `this`.
                match nodes.kind(nodes.parent_id(parent)) {
                    AstKind::CallExpression(call) if call.callee.without_parentheses().span() == member_span => return None,
                    AstKind::NewExpression(_) | AstKind::TaggedTemplateExpression(_) => return None,
                    _ => {}
                }
                if name == key && !self.read_position(parent) {
                    return None;
                }
            }
        }
        value
    }

    /// The expression node is only read where it stands (not an
    /// assignment, update, `delete` or destructuring target).
    fn read_position(&self, node: NodeId) -> bool {
        let nodes = self.parse.nodes;
        let span = nodes.kind(node).span();
        let parent = nodes.parent_id(node);
        match nodes.kind(parent) {
            AstKind::ParenthesizedExpression(_) => self.read_position(parent),
            AstKind::UnaryExpression(unary) => unary.operator != UnaryOperator::Delete,
            AstKind::AssignmentExpression(assignment) => assignment.right.without_parentheses().span() == span,
            AstKind::BinaryExpression(_)
            | AstKind::LogicalExpression(_)
            | AstKind::ConditionalExpression(_)
            | AstKind::VariableDeclarator(_)
            | AstKind::ReturnStatement(_)
            | AstKind::IfStatement(_)
            | AstKind::ExpressionStatement(_)
            | AstKind::SequenceExpression(_)
            | AstKind::TemplateLiteral(_)
            | AstKind::CallExpression(_)
            | AstKind::StaticMemberExpression(_)
            | AstKind::ComputedMemberExpression(_) => true,
            _ => false,
        }
    }

    fn sym_identifier(&self, node: NodeId, identifier: &IdentifierReference<'_>, resolver: &mut Resolver) -> Sym {
        let Some(symbol) = self.identifier_symbol(identifier) else {
            return match identifier.name.as_str() {
                "undefined" => Sym::Undef,
                name => Sym::Global(name.to_string()),
            };
        };
        if let Some(value) = resolver.resolved.get(&node) {
            return value.clone();
        }
        if resolver.hops == 0 {
            return Sym::Opaque("hops");
        }
        let Some((_, stores)) = self.reaching(node) else {
            return Sym::Opaque("flow");
        };
        let root = self.flow_root(node);
        resolver.hops -= 1;
        let mut values = Vec::new();
        for store in stores {
            match store {
                Store::Entry => values.push(self.sym_entry(symbol, root, resolver)),
                store => values.push(self.sym_store(&store, resolver)),
            }
        }
        resolver.hops += 1;
        if values.is_empty() {
            return Sym::Opaque("no store");
        }
        let value = if resolver.reads {
            Sym::Read(symbol, Box::new(Sym::phi(values)))
        } else {
            Sym::phi(values)
        };
        if !value.has_cut() {
            resolver.resolved.insert(node, value.clone());
        }
        value
    }

    /// The value of a binding when the function `root` starts.
    fn sym_entry(&self, symbol: SymbolId, root: NodeId, resolver: &mut Resolver) -> Sym {
        let scoping = self.parse.scoping;
        let declaration = scoping.symbol_declaration(symbol);
        if let Some((function, index)) = self.formal_of(declaration) {
            return Sym::Param(function, index);
        }
        match self.parse.nodes.kind(declaration) {
            AstKind::Function(_) => return Sym::Func(declaration),
            AstKind::Class(_) => return Sym::Opaque("class"),
            _ => {}
        }
        if self.flow_root(declaration) == root {
            // A binding of this function read before any store.
            return Sym::Undef;
        }
        if let Some(value) = resolver.captured.get(&(symbol, root)) {
            return value.clone();
        }
        let stores = self.outer_stores(symbol, root);
        if stores.is_empty() {
            return Sym::Opaque("captured");
        }
        let values = stores.iter().map(|store| self.sym_store(store, resolver)).collect();
        let value = Sym::phi(values);
        if !value.has_cut() {
            resolver.captured.insert((symbol, root), value.clone());
        }
        value
    }

    fn sym_store(&self, store: &Store, resolver: &mut Resolver) -> Sym {
        if resolver.active.contains(store) {
            return Sym::Opaque("cycle");
        }
        let nodes = self.parse.nodes;
        resolver.active.push(store.clone());
        let value = match store {
            Store::Entry => Sym::Opaque("entry"),
            Store::Assign(node) => match nodes.kind(*node) {
                AstKind::AssignmentExpression(assignment) => {
                    let value = self.sym_expression(&assignment.right, resolver);
                    self.guarded(*node, value, resolver)
                }
                _ => Sym::Opaque("store"),
            },
            Store::Declare(node) => match nodes.kind(*node) {
                AstKind::VariableDeclarator(declarator) => match &declarator.init {
                    Some(init) => {
                        let value = self.sym_expression(init, resolver);
                        self.guarded(*node, value, resolver)
                    }
                    None => Sym::Undef,
                },
                _ => Sym::Opaque("store"),
            },
            Store::Other(node) => match nodes.kind(*node) {
                AstKind::AssignmentExpression(assignment)
                    if assignment.operator == oxc_ast::ast::AssignmentOperator::BitwiseOR =>
                {
                    // `x |= v`: the earlier value or-ed with `v`.
                    let before = match &assignment.left {
                        AssignmentTarget::AssignmentTargetIdentifier(identifier) => {
                            self.sym_identifier(identifier.node_id.get(), identifier, resolver)
                        }
                        _ => Sym::Opaque("target"),
                    };
                    let right = self.sym_expression(&assignment.right, resolver);
                    Sym::Bin(BinaryOperator::BitwiseOR.as_str(), Box::new(before), Box::new(right))
                }
                _ => Sym::Opaque("update"),
            },
        };
        resolver.active.pop();
        value
    }

    /// A `true` store: the test it is stored under.
    fn guarded(&self, store: NodeId, value: Sym, resolver: &mut Resolver) -> Sym {
        if value != Sym::Bool(true) {
            return value;
        }
        let nodes = self.parse.nodes;
        let mut child = store;
        loop {
            let parent = nodes.parent_id(child);
            if parent == child {
                return value;
            }
            let span = nodes.kind(child).span();
            let found = match nodes.kind(parent) {
                AstKind::IfStatement(statement) => {
                    if statement.consequent.span() == span {
                        Some((&statement.test, true))
                    } else if statement.alternate.as_ref().is_some_and(|alternate| alternate.span() == span) {
                        Some((&statement.test, false))
                    } else {
                        None
                    }
                }
                AstKind::ConditionalExpression(conditional) => {
                    if conditional.consequent.span() == span {
                        Some((&conditional.test, true))
                    } else if conditional.alternate.span() == span {
                        Some((&conditional.test, false))
                    } else {
                        None
                    }
                }
                AstKind::LogicalExpression(logical) if logical.right.span() == span => {
                    Some((&logical.left, logical.operator == LogicalOperator::And))
                }
                AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) => return value,
                _ => None,
            };
            if let Some((test, polarity)) = found {
                let test = self.sym_expression(test, resolver);
                return Sym::Guard(Box::new(test), polarity);
            }
            child = parent;
        }
    }

    /// The formal a function returns unchanged on every path: plain
    /// formals, a body ending in `return p;` with no other `return` (nested
    /// functions aside), and `p` never written.
    fn returned_formal(&self, function: NodeId) -> Option<usize> {
        let (params, statements) = match self.parse.nodes.kind(function) {
            AstKind::Function(function) => (&function.params, &function.body.as_ref()?.statements),
            AstKind::ArrowFunctionExpression(arrow) if !arrow.expression => (&arrow.params, &arrow.body.statements),
            _ => return None,
        };
        if params.rest.is_some() {
            return None;
        }
        let mut formals = Vec::new();
        for parameter in &params.items {
            let oxc_ast::ast::BindingPattern::BindingIdentifier(identifier) = &parameter.pattern else {
                return None;
            };
            if parameter.initializer.is_some() {
                return None;
            }
            formals.push(identifier.symbol_id.get()?);
        }
        let (Statement::ReturnStatement(last), body) = statements.split_last()? else {
            return None;
        };
        let Expression::Identifier(returned) = last.argument.as_ref()?.without_parentheses() else {
            return None;
        };
        let symbol = crate::oxc::identifier_symbol(returned, self.parse.scoping)?;
        let index = formals.iter().position(|formal| *formal == symbol)?;
        if self.parse.scoping.get_resolved_references(symbol).any(|reference| reference.is_write()) {
            return None;
        }
        struct Returns(bool);
        impl<'a> Visit<'a> for Returns {
            fn visit_return_statement(&mut self, _: &ReturnStatement<'a>) {
                self.0 = true;
            }
            fn visit_function(&mut self, _: &Function<'a>, _: ScopeFlags) {}
            fn visit_arrow_function_expression(&mut self, _: &ArrowFunctionExpression<'a>) {}
            fn visit_class(&mut self, _: &Class<'a>) {}
        }
        let mut returns = Returns(false);
        for statement in body {
            returns.visit_statement(statement);
        }
        (!returns.0).then_some(index)
    }

    /// `(function, index)` when `declaration` is a formal parameter.
    fn formal_of(&self, declaration: NodeId) -> Option<(NodeId, usize)> {
        let nodes = self.parse.nodes;
        let mut node = declaration;
        for _ in 0..4 {
            if let AstKind::FormalParameter(_) = nodes.kind(node) {
                let parent = nodes.parent_id(node);
                let AstKind::FormalParameters(parameters) = nodes.kind(parent) else {
                    return None;
                };
                let span = nodes.kind(node).span();
                let index = parameters.items.iter().position(|item| item.span == span)?;
                return Some((nodes.parent_id(parent), index));
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

/// `String.fromCharCode.apply(null, [n, …])` and
/// `String.fromCharCode(n, …)`: the string.
fn fold_call(callee: Sym, args: Vec<Sym>) -> Sym {
    let chars = |items: &[Sym]| -> Option<String> {
        items
            .iter()
            .map(|item| match item {
                Sym::Num(code) if *code >= 0.0 && *code < 65536.0 && code.fract() == 0.0 => char::from_u32(*code as u32),
                _ => None,
            })
            .collect()
    };
    let from_char_code = |sym: &Sym| matches!(sym, Sym::Member(object, name) if name == "fromCharCode" && matches!(object.as_ref(), Sym::Global(global) if global == "String"));
    if let Sym::Member(inner, method) = &callee {
        if method == "apply" && from_char_code(inner) {
            if let [_, Sym::Array(items)] = args.as_slice() {
                if let Some(text) = chars(items) {
                    return Sym::Str(text);
                }
            }
        }
    }
    if from_char_code(&callee) {
        if let Some(text) = chars(&args) {
            return Sym::Str(text);
        }
    }
    Sym::Call(Box::new(callee), args)
}

// ===========================================================================
// Splitting a packed byte
// ===========================================================================

/// The booleans of a packed byte, by bit: the alternatives that set the bit
/// (`None`: a constant 1 bit).
pub(crate) type SymTerms = std::collections::BTreeMap<u32, Vec<Option<Sym>>>;

fn power_of_two(value: f64) -> Option<u32> {
    (value >= 1.0 && value.fract() == 0.0 && (value as u64).is_power_of_two()).then(|| (value as u64).trailing_zeros())
}

/// Split a packed value: `|` / `+` terms, `x << n`, `x * 2^n`,
/// `t ? 2^n : 0`, constant bits; a phi splits each alternative and merges
/// per bit (a bit an alternative does not set is 0 there).
pub(crate) fn split(sym: &Sym) -> Result<SymTerms, String> {
    let mut out = SymTerms::new();
    split_into(sym, 0, &mut out)?;
    for (bit, terms) in &out {
        if *bit > 7 {
            return Err(format!("bit {bit} is outside the byte"));
        }
        if terms.iter().any(Option::is_none) && terms.len() > 1 {
            return Err(format!("constant and boolean on bit {bit}"));
        }
    }
    Ok(out)
}

fn split_into(sym: &Sym, shift: u32, out: &mut SymTerms) -> Result<(), String> {
    match sym {
        Sym::Bin(op, left, right) if *op == "|" || *op == "+" => {
            // Two terms of one alternative on one bit are an error; merge
            // the halves separately.
            let mut left_terms = SymTerms::new();
            split_into(left, shift, &mut left_terms)?;
            let mut right_terms = SymTerms::new();
            split_into(right, shift, &mut right_terms)?;
            for (bit, terms) in right_terms {
                // `x | (x ? 1 : 0)`: the same boolean twice on one bit.
                if left_terms.get(&bit).is_some_and(|known| *known == terms && terms.iter().all(Option::is_some)) {
                    continue;
                }
                if left_terms.contains_key(&bit) {
                    return Err(format!("two terms on bit {bit}"));
                }
                left_terms.insert(bit, terms);
            }
            merge(out, left_terms);
            Ok(())
        }
        // `byte ^ stream()`: the masked byte (the keystream draw takes no
        // argument).
        Sym::Bin("^", value, mask) if matches!(peel_reads(mask), Sym::Call(_, args) if args.is_empty()) => {
            split_into(value, shift, out)
        }
        // `false << n` is 0 whatever `n` is.
        Sym::Bin("<<", value, _) if peel_reads(value).is_unset() => Ok(()),
        Sym::Bin("<<", value, amount) => {
            // The shift count is taken mod 32; `x << (t ? 97 : 1)` shifts
            // by 1 either way.
            let count = |sym: &Sym| match sym {
                Sym::Num(amount) if amount.fract() == 0.0 && *amount >= 0.0 && *amount < 4294967296.0 => {
                    Some((*amount as u64 & 31) as u32)
                }
                _ => None,
            };
            let amount = match amount.as_ref() {
                Sym::Cond(test, consequent, alternate) if truth(test).is_some() => {
                    count(if truth(test) == Some(true) { consequent } else { alternate })
                }
                Sym::Cond(_, consequent, alternate) => match (count(consequent), count(alternate)) {
                    (Some(a), Some(b)) if a == b => Some(a),
                    _ => None,
                },
                other => count(other),
            };
            match amount {
                Some(amount) => split_into(value, shift + amount, out),
                None => Err("variable shift".into()),
            }
        }
        Sym::Bin("*", left, right) => match (right.as_ref(), left.as_ref()) {
            (Sym::Num(value), _) if power_of_two(*value).is_some() => {
                split_into(left, shift + power_of_two(*value).unwrap_or(0), out)
            }
            (_, Sym::Num(value)) if power_of_two(*value).is_some() => {
                split_into(right, shift + power_of_two(*value).unwrap_or(0), out)
            }
            _ => Err("product".into()),
        },
        Sym::Num(value) => {
            if value.fract() != 0.0 || !(0.0..256.0).contains(value) {
                return Err(format!("constant {value}"));
            }
            let value = *value as u32;
            let mut terms = SymTerms::new();
            for bit in 0..8 {
                if value & (1 << bit) != 0 {
                    terms.insert(shift + bit, vec![None]);
                }
            }
            merge(out, terms);
            Ok(())
        }
        Sym::Bool(true) => {
            merge(out, SymTerms::from([(shift, vec![None])]));
            Ok(())
        }
        // A function in a bitwise term is NaN there: 0 (`x | f`).
        Sym::Func(_) => Ok(()),
        unset if unset.is_unset() => Ok(()),
        // A test on literals only (`typeof false !== "object"`): the arm
        // it takes.
        Sym::Cond(test, consequent, alternate) if truth(test).is_some() => {
            split_into(if truth(test) == Some(true) { consequent } else { alternate }, shift, out)
        }
        Sym::Cond(test, consequent, alternate) => match (consequent.as_ref(), alternate.as_ref()) {
            (Sym::Num(value), Sym::Num(zero)) if *zero == 0.0 && power_of_two(*value).is_some() => {
                let bit = shift + power_of_two(*value).unwrap_or(0);
                merge(out, SymTerms::from([(bit, vec![Some(test.as_ref().clone())])]));
                Ok(())
            }
            (Sym::Num(_), Sym::Num(_)) => Err("conditional constant".into()),
            _ => {
                // Arms that are packed values themselves: the byte is
                // either arm (like a phi); one boolean either way is the
                // term.
                let mut consequent_terms = SymTerms::new();
                let mut alternate_terms = SymTerms::new();
                let packed = split_into(consequent, shift, &mut consequent_terms).is_ok()
                    && split_into(alternate, shift, &mut alternate_terms).is_ok()
                    && [&consequent_terms, &alternate_terms]
                        .iter()
                        .any(|terms| terms.keys().any(|bit| *bit != shift) || terms.values().any(|items| items.len() > 1));
                if packed {
                    merge(out, consequent_terms);
                    merge(out, alternate_terms);
                } else {
                    merge(out, SymTerms::from([(shift, vec![Some(sym.clone())])]));
                }
                Ok(())
            }
        },
        Sym::Phi(items) => {
            // Each alternative splits on its own; the byte is their merge.
            let mut merged = SymTerms::new();
            for item in items {
                let mut terms = SymTerms::new();
                split_into(item, shift, &mut terms)?;
                merge(&mut merged, terms);
            }
            merge(out, merged);
            Ok(())
        }
        Sym::Opaque(reason) => Err(format!("value not followed ({reason})")),
        Sym::Read(_, value) => {
            // A read holding a packed value splits through; a read holding
            // one boolean stays the term.
            let mut terms = SymTerms::new();
            split_into(value, shift, &mut terms)?;
            let single = terms.len() == 1
                && terms.get(&shift).is_some_and(|items| items.len() == 1 && items[0].is_some())
                && !matches!(value.as_ref(), Sym::Bin(op, _, _) if matches!(*op, "|" | "+" | "<<" | "*"));
            if single {
                merge(out, SymTerms::from([(shift, vec![Some(sym.clone())])]));
            } else {
                merge(out, terms);
            }
            Ok(())
        }
        other => {
            merge(out, SymTerms::from([(shift, vec![Some(other.clone())])]));
            Ok(())
        }
    }
}

/// The value of an expression over literals only (`typeof false`,
/// `false === "true"`, `!0`); `None` for anything else.
fn literal_value(sym: &Sym) -> Option<Sym> {
    let typeof_name = |value: &Sym| match value {
        Sym::Num(_) => Some("number"),
        Sym::Str(_) => Some("string"),
        Sym::Bool(_) => Some("boolean"),
        Sym::Undef => Some("undefined"),
        Sym::Null => Some("object"),
        _ => None,
    };
    match peel_reads(sym) {
        value @ (Sym::Num(_) | Sym::Str(_) | Sym::Bool(_) | Sym::Undef | Sym::Null) => Some(value.clone()),
        Sym::Unary("typeof", value) => match literal_value(value) {
            Some(value) => Some(Sym::Str(typeof_name(&value)?.to_string())),
            None => is_boolean(value).then(|| Sym::Str("boolean".to_string())),
        },
        // A boolean is never `null`, `undefined`, a number or a string.
        Sym::Bin(op @ ("===" | "!==" | "==" | "!="), left, right)
            if literal_value(left).is_none() != literal_value(right).is_none() =>
        {
            let (value, literal) = match literal_value(left) {
                Some(literal) => (right, literal),
                None => (left, literal_value(right)?),
            };
            let never = match (*op, &literal) {
                (_, Sym::Null | Sym::Undef) => true,
                ("===" | "!==", Sym::Num(_) | Sym::Str(_)) => true,
                _ => false,
            };
            (never && is_boolean(value)).then(|| Sym::Bool(op.starts_with('!')))
        }
        Sym::Unary("!", value) => Some(Sym::Bool(!truthy(&literal_value(value)?)?)),
        Sym::Bin(op @ ("===" | "!=="), left, right) => {
            let (left, right) = (literal_value(left)?, literal_value(right)?);
            // NaN is never equal to itself.
            let equal = match (&left, &right) {
                (Sym::Num(a), Sym::Num(b)) => a == b,
                (Sym::Str(a), Sym::Str(b)) => a == b,
                (Sym::Bool(a), Sym::Bool(b)) => a == b,
                (Sym::Undef, Sym::Undef) | (Sym::Null, Sym::Null) => true,
                _ => false,
            };
            Some(Sym::Bool(equal == (*op == "===")))
        }
        Sym::Cond(test, consequent, alternate) => match truth(test)? {
            true => literal_value(consequent),
            false => literal_value(alternate),
        },
        _ => None,
    }
}

/// A value that is a boolean on every path: a boolean literal, a
/// comparison, a negation, a `true` stored under a test.
fn is_boolean(sym: &Sym) -> bool {
    match sym {
        Sym::Bool(_) | Sym::Guard(_, _) | Sym::Unary("!", _) => true,
        Sym::Bin(op, _, _) => {
            matches!(*op, "===" | "!==" | "==" | "!=" | "<" | ">" | "<=" | ">=" | "in" | "instanceof")
        }
        Sym::Read(_, value) => is_boolean(value),
        Sym::Phi(items) => !items.is_empty() && items.iter().all(is_boolean),
        Sym::Cond(_, consequent, alternate) => is_boolean(consequent) && is_boolean(alternate),
        _ => false,
    }
}

/// Whether a test over literals or booleans only holds (`typeof b ===
/// "boolean"`, `b !== null` for a boolean `b`).
pub(crate) fn decided(test: &Sym) -> Option<bool> {
    truth(test)
}

fn truthy(value: &Sym) -> Option<bool> {
    match value {
        Sym::Num(number) => Some(*number != 0.0 && !number.is_nan()),
        Sym::Str(text) => Some(!text.is_empty()),
        Sym::Bool(value) => Some(*value),
        Sym::Undef | Sym::Null => Some(false),
        _ => None,
    }
}

/// Whether a test over literals only holds.
fn truth(test: &Sym) -> Option<bool> {
    truthy(&literal_value(test)?)
}

/// The value under read wrappers (one stored value each).
fn peel_reads(sym: &Sym) -> &Sym {
    match sym {
        Sym::Read(_, value) => peel_reads(value),
        other => other,
    }
}

fn merge(out: &mut SymTerms, terms: SymTerms) {
    for (bit, items) in terms {
        let entry = out.entry(bit).or_default();
        for item in items {
            if !entry.contains(&item) {
                entry.push(item);
            }
        }
    }
}
