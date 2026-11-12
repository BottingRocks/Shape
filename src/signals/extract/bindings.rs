//! Scope-aware binding and function indexing for the program extractor.
//!
//! The structures here never decide a semantic role. They answer mechanical
//! questions about the single Oxc parse: which symbol a reference resolves to,
//! what a binding's declaration looks like, which scope owns it, and which
//! `Function` node a symbol/span denotes. Role matching lives in
//! [`super::scanner`].

use std::collections::{BTreeMap, HashMap, HashSet};

use oxc_ast::ast::*;
use oxc_ast::AstKind;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};

use crate::signals::schema::{
    BindingKind, BindingRef, ExprRecord, FunctionRef, LiteralValue, ProgramPropertiesError,
    ProgramPropertiesLimits, ValueKind,
};

/// A resolved function declaration/expression in the parsed program.
#[derive(Clone, Copy, Debug)]
pub(crate) struct FunctionSite {
    pub node_id: oxc_semantic::NodeId,
    pub span: Span,
    pub body_span: Option<Span>,
    /// Binding the function is available under, if it had a declaration or a
    /// variable it was assigned to.
    pub binding_symbol: Option<SymbolId>,
}

/// The selected main scope plus the mechanical indexes built from the parse.
pub(crate) struct Model<'s, 'a> {
    pub source: &'s str,
    pub scoping: &'a Scoping,
    pub nodes: &'a AstNodes<'a>,
    pub main_node: oxc_semantic::NodeId,
    pub main_symbols: Vec<SymbolId>,
    pub main_sites: Vec<FunctionSite>,
    function_by_start: BTreeMap<u32, FunctionSite>,
    function_by_node: HashMap<oxc_semantic::NodeId, FunctionSite>,
    slots: ParameterSlots<'a, 'a>,
    /// Node ids (in node order) by the start of their
    /// [`Self::enclosing_function`], built on first use.
    own_nodes: std::cell::OnceCell<rustc_hash::FxHashMap<u32, Vec<oxc_semantic::NodeId>>>,
    /// Node ids (in node order) by AST type, built on first use.
    by_type: std::cell::OnceCell<Vec<Vec<oxc_semantic::NodeId>>>,
    /// Every node id sorted by (span start, id), built on first use.
    by_start: std::cell::OnceCell<Vec<(u32, oxc_semantic::NodeId)>>,
    /// `return` statement ids (in node order) by their [`Self::owning_site`]
    /// node, built on first use.
    returns_by_owner:
        std::cell::OnceCell<rustc_hash::FxHashMap<oxc_semantic::NodeId, Vec<oxc_semantic::NodeId>>>,
}

impl<'s, 'a> Model<'s, 'a> {
    pub(crate) fn new(
        source: &'s str,
        program: &'a Program<'a>,
        scoping: &'a Scoping,
        nodes: &'a AstNodes<'a>,
        limits: ProgramPropertiesLimits,
    ) -> Result<Self, ProgramPropertiesError> {
        if nodes.len() > limits.max_ast_nodes {
            return Err(ProgramPropertiesError::Limit {
                what: "AST nodes",
                limit: limits.max_ast_nodes,
            });
        }
        if scoping.symbols_len() > limits.max_bindings {
            return Err(ProgramPropertiesError::Limit {
                what: "bindings",
                limit: limits.max_bindings,
            });
        }

        let mut scope_by_node = HashMap::new();
        for scope in scoping.scope_descendants_from_root() {
            scope_by_node.insert(scoping.get_node_id(scope), scope);
        }

        let mut function_by_start = BTreeMap::new();
        let mut function_by_node = HashMap::new();
        let mut sites = Vec::new();
        for (node_id, node) in nodes.iter_enumerated() {
            let (span, body_span, binding_symbol) = match node.kind() {
                AstKind::Function(function) => {
                    let Some(body) = function.body.as_ref() else {
                        continue;
                    };
                    let binding_symbol = function
                        .id
                        .as_ref()
                        .and_then(|id| id.symbol_id.get())
                        .or_else(|| {
                            variable_declarator_binding(nodes, scoping, node_id, function.span)
                        });
                    (function.span, Some(body.span), binding_symbol)
                }
                // Arrows are producer callbacks as often as they are ordinary
                // helper functions. They get a mechanical FunctionSite so a
                // caller can resolve a callback argument by exact span, but
                // they are deliberately not added to the call-reachability set
                // below: an arrow is not a candidate main scope unless a
                // declaration names it for an ordinary call.
                AstKind::ArrowFunctionExpression(arrow) => {
                    let binding_symbol =
                        variable_declarator_binding(nodes, scoping, node_id, arrow.span);
                    (arrow.span, Some(arrow.body.span), binding_symbol)
                }
                _ => continue,
            };
            if sites.len() >= limits.max_functions {
                return Err(ProgramPropertiesError::Limit {
                    what: "functions",
                    limit: limits.max_functions,
                });
            }
            let site = FunctionSite {
                node_id,
                span,
                body_span,
                binding_symbol,
            };
            function_by_start.insert(span.start, site);
            function_by_node.insert(node_id, site);
            sites.push(site);
        }

        let callable = called_function_spans(scoping, nodes, &function_by_start, &sites);
        let mut best: Option<(usize, FunctionSite)> = None;
        let mut tied: Vec<FunctionSite> = Vec::new();
        for site in &sites {
            if !callable.contains(&site.span.start) {
                continue;
            }
            let Some(scope) = scope_by_node.get(&site.node_id).copied() else {
                continue;
            };
            let count = scoping.iter_bindings_in(scope).count();
            match &best {
                None => best = Some((count, *site)),
                Some((best_count, best_site)) => {
                    if count > *best_count {
                        best = Some((count, *site));
                        tied.clear();
                    } else if count == *best_count && site.span != best_site.span {
                        tied.push(*site);
                    }
                }
            }
        }

        if !tied.is_empty() {
            let candidates = tied
                .iter()
                .map(|site| format!("{}..{}", site.span.start, site.span.end))
                .collect();
            return Err(ProgramPropertiesError::Ambiguous {
                what: "main program scope (equal broadest reachable binding set)",
                candidates,
            });
        }
        let (_, main_site) = best.ok_or(ProgramPropertiesError::Missing {
            what: "reachable main program scope",
        })?;
        let main_scope = scope_by_node.get(&main_site.node_id).copied().ok_or(
            ProgramPropertiesError::Missing {
                what: "main function scope",
            },
        )?;
        let main_symbols: Vec<SymbolId> = scoping.iter_bindings_in(main_scope).collect();

        Ok(Self {
            source,
            scoping,
            nodes,
            main_node: main_site.node_id,
            main_symbols,
            main_sites: sites,
            function_by_start,
            function_by_node,
            slots: ParameterSlots::new(program, scoping, nodes),
            own_nodes: std::cell::OnceCell::new(),
            by_start: std::cell::OnceCell::new(),
            by_type: std::cell::OnceCell::new(),
            returns_by_owner: std::cell::OnceCell::new(),
        })
    }

    /// The nodes of one AST type, in node order: what a whole-program scan
    /// that skips every other kind visits.
    pub(crate) fn kind_nodes(
        &self,
        ty: oxc_ast::AstType,
    ) -> impl Iterator<Item = &'a oxc_semantic::AstNode<'a>> + '_ {
        let by_type = self.by_type.get_or_init(|| {
            let mut by_type = vec![Vec::new(); oxc_ast::ast_kind::AST_TYPE_MAX as usize + 1];
            for node in self.nodes.iter() {
                by_type[node.kind().ty() as usize].push(node.id());
            }
            by_type
        });
        let nodes = self.nodes;
        by_type[ty as usize].iter().map(move |id| nodes.get_node(*id))
    }

    /// The nodes for which [`Self::node_within`]`(node, site)` holds, in node
    /// order: what a whole-program scan filtered on it visits.
    pub(crate) fn nodes_within(&self, site: FunctionSite) -> Vec<&'a oxc_semantic::AstNode<'a>> {
        let by_start = self.by_start.get_or_init(|| {
            let mut all: Vec<(u32, oxc_semantic::NodeId)> =
                self.nodes.iter().map(|node| (node.span().start, node.id())).collect();
            all.sort_unstable();
            all
        });
        let body = site.body_span.unwrap_or(site.span);
        let first = by_start.partition_point(|(start, _)| *start < body.start);
        let mut ids: Vec<oxc_semantic::NodeId> = by_start[first..]
            .iter()
            .take_while(|(start, _)| *start <= body.end)
            .filter(|(_, id)| self.nodes.get_node(*id).span().end <= body.end)
            .map(|(_, id)| *id)
            .collect();
        ids.sort_unstable();
        ids.into_iter().map(|id| self.nodes.get_node(id)).collect()
    }

    /// The `return` statements whose [`Self::owning_site`] is `site`, in node
    /// order.
    pub(crate) fn owned_returns(
        &self,
        site: FunctionSite,
    ) -> impl Iterator<Item = &'a oxc_semantic::AstNode<'a>> + '_ {
        let index = self.returns_by_owner.get_or_init(|| {
            let mut index: rustc_hash::FxHashMap<oxc_semantic::NodeId, Vec<oxc_semantic::NodeId>> =
                rustc_hash::FxHashMap::default();
            for node in self.nodes.iter() {
                if !matches!(node.kind(), AstKind::ReturnStatement(_)) {
                    continue;
                }
                if let Some(owner) = self.owning_site(node.id()) {
                    index.entry(owner.node_id).or_default().push(node.id());
                }
            }
            index
        });
        let nodes = self.nodes;
        index
            .get(&site.node_id)
            .map(Vec::as_slice)
            .unwrap_or(&[])
            .iter()
            .map(move |id| nodes.get_node(*id))
    }

    /// The nodes whose [`Self::enclosing_function`] starts where `site`
    /// starts, in node order: what a whole-program scan filtered on
    /// `enclosing_function(node).span.start == site.span.start` visits.
    pub(crate) fn own_nodes(
        &self,
        site: FunctionSite,
    ) -> impl Iterator<Item = &'a oxc_semantic::AstNode<'a>> + '_ {
        let index = self.own_nodes.get_or_init(|| {
            let mut index: rustc_hash::FxHashMap<u32, Vec<oxc_semantic::NodeId>> =
                rustc_hash::FxHashMap::default();
            for node in self.nodes.iter() {
                if let Some(owner) = self.enclosing_function(node.id()) {
                    index.entry(owner.span.start).or_default().push(node.id());
                }
            }
            index
        });
        let nodes = self.nodes;
        index
            .get(&site.span.start)
            .map(Vec::as_slice)
            .unwrap_or(&[])
            .iter()
            .map(move |id| nodes.get_node(*id))
    }

    pub(crate) fn main_site(&self) -> FunctionSite {
        self.function_by_node[&self.main_node]
    }

    pub(crate) fn all_sites(&self) -> &[FunctionSite] {
        &self.main_sites
    }

    pub(crate) fn site_by_span(&self, span: Span) -> Option<FunctionSite> {
        self.function_by_start.get(&span.start).copied()
    }

    /// Resolve an expression that denotes a function, including a chain of
    /// plain `var alias = target` aliases.
    pub(crate) fn function_from_expression(
        &self,
        expression: &Expression<'_>,
    ) -> Option<FunctionSite> {
        match expression {
            Expression::FunctionExpression(function) => self.site_by_span(function.span),
            Expression::ArrowFunctionExpression(arrow) => self.site_by_span(arrow.span),
            Expression::Identifier(identifier) => self.function_from_symbol(
                crate::analysis::identifier_symbol(identifier, self.scoping)?,
            ),
            _ => None,
        }
    }

    /// Resolve a function value expression through an identifier that holds a
    /// function literal directly. This is the narrow form used when the caller
    /// requires a function argument and must not accept a factory call.
    fn function_literal_from_value(
        &self,
        value: &Expression<'a>,
        depth: usize,
    ) -> Option<FunctionSite> {
        if depth > 16 {
            return None;
        }
        match value {
            Expression::FunctionExpression(function) => self.site_by_span(function.span),
            Expression::ArrowFunctionExpression(arrow) => self.site_by_span(arrow.span),
            Expression::Identifier(identifier) => {
                let alias = crate::analysis::identifier_symbol(identifier, self.scoping)?;
                let declaration = self.nodes.get_node(self.scoping.symbol_declaration(alias));
                match declaration.kind() {
                    AstKind::Function(function) => self.site_by_span(function.span),
                    AstKind::VariableDeclarator(declarator) => {
                        let value = declarator
                            .init
                            .as_ref()
                            .or_else(|| self.assigned_value(alias))?;
                        self.function_literal_from_value(value, depth + 1)
                    }
                    _ => {
                        let value = self.assigned_value(alias)?;
                        self.function_literal_from_value(value, depth + 1)
                    }
                }
            }
            _ => None,
        }
    }

    /// Resolve a symbol to the function it holds, following at most a small
    /// alias chain. This deliberately refuses cycles and non-function values.
    pub(crate) fn function_from_symbol(&self, symbol: SymbolId) -> Option<FunctionSite> {
        self.function_from_symbol_inner(symbol, 0)
    }

    fn function_from_symbol_inner(&self, symbol: SymbolId, depth: usize) -> Option<FunctionSite> {
        if depth > 16 {
            return None;
        }
        let declaration = self.nodes.get_node(self.scoping.symbol_declaration(symbol));
        if let AstKind::Function(function) = declaration.kind() {
            return self.site_by_span(function.span);
        }
        // A later direct assignment is the effective binding.  If that value
        // is not a proven function, refuse the old initializer rather than
        // returning a stale callable through an unsupported mutation.
        if let Some(value) = self.assigned_value(symbol) {
            return self.function_literal_from_value(value, depth + 1);
        }
        let initializer = self.initializer(symbol)?;
        self.function_literal_from_value(initializer, depth + 1)
    }

    /// The last direct `x = expression` assignment to a binding. Unlike
    /// [`Self::symbol_value`] this never prefers the declarator initializer, so
    /// a recovered descriptor table can be resolved through its later
    /// `table[index] = object` population writes.
    pub(crate) fn symbol_assigned_value(&self, symbol: SymbolId) -> Option<&'a Expression<'a>> {
        self.assigned_value(symbol)
    }

    /// The function symbol a call/new expression targets, when it is a plain
    /// identifier or function expression.
    pub(crate) fn callee_function(&self, callee: &Expression<'_>) -> Option<FunctionSite> {
        self.function_from_expression(callee)
    }

    pub(crate) fn calls_to(&self, site: FunctionSite) -> Vec<&'a CallExpression<'a>> {
        let mut calls = Vec::new();
        for node in self.nodes.iter() {
            let AstKind::CallExpression(call) = node.kind() else {
                continue;
            };
            if let Some(target) = self.callee_function(&call.callee) {
                if target.span.start == site.span.start {
                    calls.push(call);
                }
            }
        }
        calls
    }

    pub(crate) fn new_expressions_to(&self, site: FunctionSite) -> Vec<&'a NewExpression<'a>> {
        let mut calls = Vec::new();
        for node in self.nodes.iter() {
            let AstKind::NewExpression(call) = node.kind() else {
                continue;
            };
            if let Some(target) = self.callee_function(&call.callee) {
                if target.span.start == site.span.start {
                    calls.push(call);
                }
            }
        }
        calls
    }

    pub(crate) fn symbol_by_index(&self, index: u32) -> Option<SymbolId> {
        self.scoping
            .symbol_ids()
            .find(|symbol| symbol.index() as u32 == index)
    }

    pub(crate) fn binding_ref(&self, symbol: SymbolId) -> BindingRef {
        let owned = self.main_symbols.contains(&symbol);
        BindingRef {
            symbol: symbol.index() as u32,
            name: self.scoping.symbol_name(symbol).to_string(),
            declaration: SourceSpanExt::from_span(
                self.nodes
                    .get_node(self.scoping.symbol_declaration(symbol))
                    .span(),
            ),
            identifier: SourceSpanExt::from_span(self.scoping.symbol_span(symbol)),
            owned_by_main: owned,
        }
    }

    pub(crate) fn binding_kind(&self, symbol: SymbolId) -> BindingKind {
        match self
            .nodes
            .get_node(self.scoping.symbol_declaration(symbol))
            .kind()
        {
            AstKind::VariableDeclarator(_) => BindingKind::Variable,
            AstKind::Function(_) => BindingKind::Function,
            AstKind::FormalParameter(_) => BindingKind::Parameter,
            AstKind::CatchParameter(_) => BindingKind::Catch,
            AstKind::Class(_) => BindingKind::Class,
            _ => BindingKind::Other,
        }
    }

    pub(crate) fn binding_writes(&self, symbol: SymbolId) -> usize {
        self.scoping
            .get_resolved_references(symbol)
            .filter(|reference| reference.is_write())
            .count()
    }

    pub(crate) fn binding_references(&self, symbol: SymbolId) -> usize {
        self.scoping.get_resolved_reference_ids(symbol).len()
    }

    pub(crate) fn initializer(&self, symbol: SymbolId) -> Option<&'a Expression<'a>> {
        match self
            .nodes
            .get_node(self.scoping.symbol_declaration(symbol))
            .kind()
        {
            AstKind::VariableDeclarator(declarator) => declarator.init.as_ref(),
            _ => None,
        }
    }

    /// The effective source value of a binding: its declarator initializer or,
    /// when the recovered form declares the local up front and assigns later,
    /// the last direct `x = expression` write in the program.
    pub(crate) fn symbol_value(&self, symbol: SymbolId) -> Option<&'a Expression<'a>> {
        if let Some(initializer) = self.initializer(symbol) {
            return Some(initializer);
        }
        self.assigned_value(symbol)
    }

    fn assigned_value(&self, symbol: SymbolId) -> Option<&'a Expression<'a>> {
        let mut best: Option<(u32, &'a Expression<'a>)> = None;
        // The semantic reference list is already scoped to this symbol. Walking
        // the entire AST here made alias resolution quadratic when wiring asks
        // the same question for every emitter/coordinator candidate.
        for reference in self.scoping.get_resolved_references(symbol) {
            let AstKind::AssignmentExpression(assignment) =
                self.nodes.parent_kind(reference.node_id())
            else {
                continue;
            };
            if assignment.operator != oxc_syntax::operator::AssignmentOperator::Assign {
                continue;
            }
            let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left else {
                continue;
            };
            if crate::analysis::identifier_symbol(identifier, self.scoping) != Some(symbol) {
                continue;
            }
            if best.is_none_or(|(start, _)| assignment.span.start >= start) {
                best = Some((assignment.span.start, &assignment.right));
            }
        }
        best.map(|(_, value)| value)
    }

    pub(crate) fn function_ref(&self, site: FunctionSite) -> FunctionRef {
        let binding = site.binding_symbol.map(|symbol| self.binding_ref(symbol));
        FunctionRef {
            binding,
            name_label: site
                .binding_symbol
                .map(|symbol| self.scoping.symbol_name(symbol).to_string()),
            span: SourceSpanExt::from_span(site.span),
            body: site.body_span.map(SourceSpanExt::from_span),
            source: None,
        }
    }

    pub(crate) fn exact(&self, span: Span) -> &'s str {
        &self.source[span.start as usize..span.end as usize]
    }

    pub(crate) fn expr_record(&self, expression: &Expression<'a>) -> ExprRecord {
        let span = expression.span();
        let binding = match expression {
            Expression::Identifier(identifier) => {
                crate::analysis::identifier_symbol(identifier, self.scoping)
                    .map(|symbol| self.binding_ref(symbol))
            }
            _ => None,
        };
        ExprRecord {
            span: SourceSpanExt::from_span(span),
            source: self.exact(span).to_string(),
            kind: value_kind(expression),
            literal: literal_value_recursive(expression),
            binding,
            function: self
                .function_from_expression(expression)
                .map(|site| self.function_ref(site)),
        }
    }

    /// True when `node_id` is physically inside `site`'s body, including nested
    /// callbacks (an emission can be raised from a callback created by a
    /// collector).
    pub(crate) fn node_within(&self, node_id: oxc_semantic::NodeId, site: FunctionSite) -> bool {
        let span = self.nodes.get_node(node_id).span();
        let body = site.body_span.unwrap_or(site.span);
        span.start >= body.start && span.end <= body.end
    }

    /// The innermost enclosing normal function of a node.
    pub(crate) fn enclosing_function(&self, node_id: oxc_semantic::NodeId) -> Option<FunctionSite> {
        let mut current = node_id;
        for _ in 0..512 {
            let parent = self.nodes.parent_id(current);
            if parent == current {
                return None;
            }
            current = parent;
            if let AstKind::Function(function) = self.nodes.kind(current) {
                return self.site_by_span(function.span);
            }
        }
        None
    }

    /// The innermost function or arrow whose own body holds a node. Unlike
    /// [`Self::enclosing_function`] an arrow is an owner here: it owns its
    /// formals and its `return` statements, though not `arguments`.
    pub(crate) fn owning_site(&self, node_id: oxc_semantic::NodeId) -> Option<FunctionSite> {
        let mut current = node_id;
        for _ in 0..512 {
            let parent = self.nodes.parent_id(current);
            if parent == current {
                return None;
            }
            current = parent;
            if matches!(
                self.nodes.kind(current),
                AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)
            ) {
                return self.function_by_node.get(&current).copied();
            }
        }
        None
    }

    /// The parameter slot an expression denotes, in either capture
    /// representation; see [`ParameterSlots::parameter_slot`]. The slot names
    /// the owning function. A caller matching one function must compare the
    /// owner with it (see [`Self::site_parameter_slot`]).
    pub(crate) fn parameter_slot(&self, expression: &Expression<'_>) -> Option<ParameterSlot> {
        self.slots.parameter_slot(expression)
    }

    /// Incoming ABI value, including the existing arguments-mutation guard.
    pub(crate) fn incoming_slot(&self, expression: &Expression<'_>) -> Option<ParameterSlot> {
        self.slots.incoming_slot(expression)
    }

    /// The slot index when `expression` denotes a parameter slot of `site`
    /// itself, never of an inner or outer function.
    pub(crate) fn site_parameter_slot(
        &self,
        expression: &Expression<'_>,
        site: FunctionSite,
    ) -> Option<u32> {
        self.parameter_slot(expression)
            .filter(|slot| slot.owner == site.node_id)
            .map(|slot| slot.index)
    }

    /// Like [`Self::site_parameter_slot`], but also accepts a local of `site`
    /// that only ever holds that slot: every write to it (its initializer and
    /// each plain `=` assignment, of which there is at least one) is the same
    /// slot of `site`, and it is never updated or compound-assigned. Emitted
    /// code commonly copies `arguments[k]` into a branch-joined local.
    pub(crate) fn site_slot_value(
        &self,
        expression: &Expression<'_>,
        site: FunctionSite,
    ) -> Option<u32> {
        if let Some(slot) = self.site_parameter_slot(expression, site) {
            return Some(slot);
        }
        let Expression::Identifier(identifier) = expression.without_parentheses() else {
            return None;
        };
        let symbol = crate::analysis::identifier_symbol(identifier, self.scoping)?;
        let declaration = self.scoping.symbol_span(symbol);
        let body = site.body_span.unwrap_or(site.span);
        if declaration.start < body.start
            || declaration.end > body.end
            || !self.scoping.symbol_redeclarations(symbol).is_empty()
            || formal_parameter_slot(self.nodes, self.scoping, symbol).is_some()
        {
            return None;
        }
        let mut slot = None;
        let mut agree = |value: &Expression<'_>| {
            let found = self.site_parameter_slot(value, site);
            if found.is_none() || (slot.is_some() && slot != found) {
                return false;
            }
            slot = found;
            true
        };
        if let Some(value) = self.initializer(symbol) {
            if !agree(value) {
                return None;
            }
        }
        for reference in self.scoping.get_resolved_references(symbol) {
            if !reference.is_write() {
                continue;
            }
            let parent = self.nodes.parent_id(reference.node_id());
            let AstKind::AssignmentExpression(assignment) = self.nodes.kind(parent) else {
                return None;
            };
            if assignment.operator != oxc_syntax::operator::AssignmentOperator::Assign
                || !agree(&assignment.right)
            {
                return None;
            }
        }
        slot
    }

    /// The parameter slot a named formal stands for; see
    /// [`ParameterSlots::formal_symbol_slot`].
    pub(crate) fn formal_symbol_slot(&self, symbol: SymbolId) -> Option<ParameterSlot> {
        self.slots.formal_symbol_slot(symbol)
    }

    pub(crate) fn scoping(&self) -> &'a Scoping {
        self.scoping
    }

    pub(crate) fn nodes(&self) -> &'a AstNodes<'a> {
        self.nodes
    }
}

// A tiny converter kept local so the public `SourceSpan` type does not need a
// public `From<Span>` dependency in this file's imports.
struct SourceSpanExt;
impl SourceSpanExt {
    fn from_span(span: Span) -> crate::signals::schema::SourceSpan {
        crate::signals::schema::SourceSpan::new(span.start, span.end)
    }
}

/// The declarator binding that directly holds a function/arrow node, as in
/// `let f = () => {}` or `let g = function () {}`. The declarator span also
/// covers the identifier, so the function is matched against the declarator's
/// `init` expression span. A function nested inside a larger initializer
/// (`let h = wrap(function () {})`) is not held by the declarator and stays
/// unbound.
fn variable_declarator_binding(
    nodes: &AstNodes<'_>,
    _scoping: &Scoping,
    node_id: oxc_semantic::NodeId,
    function_span: Span,
) -> Option<SymbolId> {
    let parent = nodes.parent_id(node_id);
    match nodes.kind(parent) {
        AstKind::VariableDeclarator(declarator)
            if declarator
                .init
                .as_ref()
                .is_some_and(|init| init.span() == function_span) =>
        {
            binding_pattern_symbol(&declarator.id)
        }
        _ => None,
    }
}

/// The function or arrow node a value expression denotes through proven
/// bindings only: a function/arrow literal, or an identifier bound once to a
/// function declaration or a function-valued declarator that nothing else
/// writes. Returns the node and its span; anything else is refused with a
/// reason.
pub(crate) fn proven_function_node<'a>(
    scoping: &Scoping,
    nodes: &AstNodes<'a>,
    expression: Option<&Expression<'a>>,
    depth: usize,
) -> Result<(oxc_semantic::NodeId, Span), String> {
    let Some(expression) = expression else {
        return Err("no expression".to_string());
    };
    if depth > 16 {
        return Err("alias chain is too deep".to_string());
    }
    match expression.without_parentheses() {
        Expression::FunctionExpression(function) => Ok((function.node_id.get(), function.span)),
        Expression::ArrowFunctionExpression(arrow) => Ok((arrow.node_id.get(), arrow.span)),
        Expression::Identifier(identifier) => {
            let Some(symbol) = crate::analysis::identifier_symbol(identifier, scoping) else {
                return Err(format!("`{}` is not a program binding", identifier.name));
            };
            if !scoping.symbol_redeclarations(symbol).is_empty() {
                return Err(format!("`{}` is declared more than once", identifier.name));
            }
            if scoping
                .get_resolved_references(symbol)
                .any(|reference| reference.is_write())
            {
                // A function declaration binding is often reassigned before the
                // read to another proven function (`f = otherTask`). Follow the
                // latest unconditional write that dominates this read; anything
                // conditional, later, or in a sibling callable stays refused.
                if let Some(value) = reaching_write_value(nodes, scoping, identifier, symbol) {
                    return proven_function_node(scoping, nodes, Some(value), depth + 1);
                }
                return Err(format!(
                    "`{}` is written after its definition",
                    identifier.name
                ));
            }
            match nodes.kind(scoping.symbol_declaration(symbol)) {
                AstKind::Function(function) => Ok((function.node_id.get(), function.span)),
                AstKind::VariableDeclarator(declarator) => {
                    proven_function_node(scoping, nodes, declarator.init.as_ref(), depth + 1)
                }
                _ => Err(format!(
                    "`{}` is not bound by a function or variable declaration",
                    identifier.name
                )),
            }
        }
        // `factory(config)` where the called function's final statement returns
        // a function value: the call itself denotes that function. This is how
        // a task factory hands the collector callback to a wrapper
        // (`wrapper(makeTask(config))`). The proof requires the single return to
        // be the last statement and every other own-level return path absent, so
        // the call cannot fall through with a non-function value.
        Expression::CallExpression(call) => {
            let (callee_node, _) =
                proven_function_node(scoping, nodes, Some(&call.callee), depth + 1)?;
            let AstKind::Function(function) = nodes.kind(callee_node) else {
                return Err("the called value is not a declared function".to_string());
            };
            let Some(body) = function.body.as_ref() else {
                return Err("the called function has no body".to_string());
            };
            let Some(Statement::ReturnStatement(last)) = body.statements.last() else {
                return Err("the called function does not end in a return".to_string());
            };
            let mut counter = OwnReturnCounter::default();
            for statement in &body.statements[..body.statements.len() - 1] {
                counter.visit_statement(statement);
            }
            if counter.count != 0 {
                return Err("the called function has another return path".to_string());
            }
            let value = last
                .argument
                .as_ref()
                .ok_or("the called function returns no value")?;
            proven_function_node(scoping, nodes, Some(value), depth + 1)
        }
        _ => Err("expression is not a function value".to_string()),
    }
}


/// The callable (function, arrow, or program) that lexically owns a node.
fn enclosing_callable(nodes: &AstNodes<'_>, mut node: oxc_semantic::NodeId) -> Option<oxc_semantic::NodeId> {
    loop {
        match nodes.kind(node) {
            AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) => return Some(node),
            AstKind::Program(_) => return None,
            _ => node = nodes.parent_id(node),
        }
    }
}

/// True when the node is evaluated unconditionally in its own callable (no
/// enclosing branch, loop, switch, `try` or short-circuit).
fn write_is_unconditional(nodes: &AstNodes<'_>, mut node: oxc_semantic::NodeId) -> bool {
    loop {
        match nodes.kind(node) {
            AstKind::IfStatement(_)
            | AstKind::WhileStatement(_)
            | AstKind::DoWhileStatement(_)
            | AstKind::ForStatement(_)
            | AstKind::ForInStatement(_)
            | AstKind::ForOfStatement(_)
            | AstKind::SwitchStatement(_)
            | AstKind::TryStatement(_)
            | AstKind::ConditionalExpression(_)
            | AstKind::LogicalExpression(_) => return false,
            AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) | AstKind::Program(_) => {
                return true;
            }
            _ => node = nodes.parent_id(node),
        }
    }
}

/// The value the latest dominating unconditional write stores, when this read
/// is reached by that write.
fn reaching_write_value<'a>(
    nodes: &AstNodes<'a>,
    scoping: &Scoping,
    identifier: &IdentifierReference<'_>,
    symbol: SymbolId,
) -> Option<&'a Expression<'a>> {
    let use_node = identifier
        .reference_id
        .get()
        .map(|id| scoping.get_reference(id).node_id())?;
    let use_callable = enclosing_callable(nodes, use_node);
    let use_start = identifier.span.start;
    let mut chosen: Option<(u32, oxc_semantic::NodeId)> = None;
    for reference in scoping.get_resolved_references(symbol) {
        if !reference.is_write() {
            continue;
        }
        let write_node = reference.node_id();
        let span = nodes.kind(write_node).span();
        if span.start >= use_start {
            continue;
        }
        let write_callable = enclosing_callable(nodes, write_node);
        let dominates = match (write_callable, use_callable) {
            (None, None) => true,
            (Some(write), Some(use_)) if write == use_ => true,
            (Some(write), Some(use_)) => {
                let write_span = nodes.kind(write).span();
                let use_span = nodes.kind(use_).span();
                write_span.start <= use_span.start
                    && use_span.start < write_span.end
                    && span.start < use_span.start
            }
            _ => false,
        };
        if !dominates || !write_is_unconditional(nodes, write_node) {
            continue;
        }
        if chosen.is_none_or(|(at, _)| span.start > at) {
            chosen = Some((span.start, write_node));
        }
    }
    let (_, write_node) = chosen?;
    match nodes.kind(nodes.parent_id(write_node)) {
        AstKind::AssignmentExpression(assignment)
            if assignment.operator == AssignmentOperator::Assign =>
        {
            Some(&assignment.right)
        }
        AstKind::VariableDeclarator(declarator) => declarator.init.as_ref(),
        _ => None,
    }
}

/// The function `symbol` holds when another reference, `read`, is evaluated:
/// for a read inside a function body that runs when that function is called
/// at `read`. The binding must be declared once without an initializer and
/// written exactly once, by a plain unconditional write that dominates `read`
/// (see [`reaching_write_value`]); the written value must be a proven function.
pub(crate) fn proven_function_at_read<'a>(
    scoping: &Scoping,
    nodes: &AstNodes<'a>,
    symbol: SymbolId,
    read: &IdentifierReference<'_>,
) -> Result<(oxc_semantic::NodeId, Span), String> {
    if !scoping.symbol_redeclarations(symbol).is_empty() {
        return Err("the binding is declared more than once".to_string());
    }
    if let AstKind::VariableDeclarator(declarator) = nodes.kind(scoping.symbol_declaration(symbol)) {
        if declarator.init.is_some() {
            return Err("the binding has an initializer and a write".to_string());
        }
    } else {
        return Err("the binding is not a variable".to_string());
    }
    let writes = scoping
        .get_resolved_references(symbol)
        .filter(|reference| reference.is_write())
        .count();
    if writes != 1 {
        return Err(format!("the binding has {writes} writes, not one"));
    }
    let value = reaching_write_value(nodes, scoping, read, symbol)
        .ok_or_else(|| "the binding's only write does not reach the read".to_string())?;
    proven_function_node(scoping, nodes, Some(value), 1)
}

/// Counts `return` statements in a function's own body, never descending into
/// nested functions (their returns belong to a different callable).
#[derive(Default)]
struct OwnReturnCounter {
    count: usize,
}

impl<'a> Visit<'a> for OwnReturnCounter {
    fn visit_function(&mut self, _function: &Function<'a>, _flags: oxc_syntax::scope::ScopeFlags) {}
    fn visit_arrow_function_expression(&mut self, _arrow: &ArrowFunctionExpression<'a>) {}
    fn visit_return_statement(&mut self, _statement: &ReturnStatement<'a>) {
        self.count += 1;
    }
}

pub(crate) fn binding_pattern_symbol(pattern: &BindingPattern<'_>) -> Option<SymbolId> {
    match pattern {
        BindingPattern::BindingIdentifier(identifier) => identifier.symbol_id.get(),
        _ => None,
    }
}

/// The function/arrow whose formal parameter declares `symbol`, plus the
/// zero-based parameter position. This is how `arguments[2]`-style positions
/// are recovered for emitted code that names its formals.
pub(crate) fn formal_parameter_slot(
    nodes: &AstNodes<'_>,
    scoping: &Scoping,
    symbol: SymbolId,
) -> Option<(oxc_semantic::NodeId, usize)> {
    let mut current = scoping.symbol_declaration(symbol);
    for _ in 0..24 {
        let node = nodes.get_node(current);
        let index = match node.kind() {
            AstKind::Function(function) => formal_index(&function.params, symbol),
            AstKind::ArrowFunctionExpression(arrow) => formal_index(&arrow.params, symbol),
            _ => None,
        };
        if let Some(index) = index {
            return Some((current, index));
        }
        let parent = nodes.parent_id(current);
        if parent == current {
            return None;
        }
        current = parent;
    }
    None
}

fn formal_index(params: &oxc_ast::ast::FormalParameters<'_>, symbol: SymbolId) -> Option<usize> {
    params
        .items
        .iter()
        .position(|param| binding_pattern_symbol(&param.pattern) == Some(symbol))
}

/// The formal parameter symbols of the function/arrow node, in source order.
pub(crate) fn function_formal_symbols(
    nodes: &AstNodes<'_>,
    node_id: oxc_semantic::NodeId,
) -> Vec<Option<SymbolId>> {
    match nodes.get_node(node_id).kind() {
        AstKind::Function(function) => function
            .params
            .items
            .iter()
            .map(|param| binding_pattern_symbol(&param.pattern))
            .collect(),
        AstKind::ArrowFunctionExpression(arrow) => arrow
            .params
            .items
            .iter()
            .map(|param| binding_pattern_symbol(&param.pattern))
            .collect(),
        _ => Vec::new(),
    }
}

fn called_function_spans(
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    by_start: &BTreeMap<u32, FunctionSite>,
    sites: &[FunctionSite],
) -> HashSet<u32> {
    let mut called = HashSet::new();
    for node in nodes.iter() {
        let callee = match node.kind() {
            AstKind::CallExpression(call) => &call.callee,
            AstKind::NewExpression(new) => &new.callee,
            _ => continue,
        };
        match callee {
            Expression::Identifier(identifier) => {
                if let Some(symbol) = crate::analysis::identifier_symbol(identifier, scoping) {
                    if let Some(site) =
                        resolve_site_for_symbol(scoping, nodes, by_start, sites, symbol)
                    {
                        called.insert(site.span.start);
                    }
                }
            }
            Expression::FunctionExpression(function) => {
                if by_start.contains_key(&function.span.start) {
                    called.insert(function.span.start);
                }
            }
            _ => {}
        }
    }
    called
}

fn resolve_site_for_symbol(
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    by_start: &BTreeMap<u32, FunctionSite>,
    sites: &[FunctionSite],
    symbol: SymbolId,
) -> Option<FunctionSite> {
    let declaration = nodes.get_node(scoping.symbol_declaration(symbol));
    match declaration.kind() {
        AstKind::Function(function) => by_start.get(&function.span.start).copied(),
        AstKind::VariableDeclarator(declarator) => match declarator.init.as_ref()? {
            Expression::FunctionExpression(function) => by_start.get(&function.span.start).copied(),
            Expression::Identifier(identifier) => {
                let alias = crate::analysis::identifier_symbol(identifier, scoping)?;
                if alias == symbol {
                    None
                } else {
                    resolve_site_for_symbol(scoping, nodes, by_start, sites, alias)
                }
            }
            _ => None,
        },
        _ => {
            // Assignment-initialized function aliases (`alias = fn`) are
            // common in this emitted shape. Callers that need them use the
            // Model methods; this free helper is only for call-reachability and
            // can conservatively ignore that form.
            let _ = sites;
            None
        }
    }
}

// ===========================================================================
// Small AST readers shared with scanner.rs
// ===========================================================================

pub(crate) struct ObjectMember<'a> {
    pub name: String,
    pub value: &'a Expression<'a>,
    pub span: Span,
}

pub(crate) fn object_members<'a>(expression: &'a Expression<'a>) -> Option<Vec<ObjectMember<'a>>> {
    let Expression::ObjectExpression(object) = expression else {
        return None;
    };
    Some(object_members_of(object))
}

pub(crate) fn object_members_of<'a>(object: &'a ObjectExpression<'a>) -> Vec<ObjectMember<'a>> {
    let mut members = Vec::new();
    for property in &object.properties {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            continue;
        };
        let Some(name) = property_key_name(&property.key) else {
            continue;
        };
        members.push(ObjectMember {
            name,
            value: &property.value,
            span: property.span,
        });
    }
    members
}

/// Literal name of a computed member access (`obj["name"]`).
pub(crate) fn computed_member_name(member: &ComputedMemberExpression<'_>) -> Option<String> {
    match &member.expression {
        Expression::StringLiteral(literal) => Some(literal.value.to_string()),
        Expression::NumericLiteral(literal) => Some(format_number(literal.value)),
        _ => None,
    }
}

pub(crate) fn property_key_name(key: &PropertyKey<'_>) -> Option<String> {
    match key {
        PropertyKey::StaticIdentifier(identifier) => Some(identifier.name.to_string()),
        PropertyKey::StringLiteral(literal) => Some(literal.value.to_string()),
        PropertyKey::NumericLiteral(literal) => Some(format_number(literal.value)),
        PropertyKey::Identifier(identifier) => Some(identifier.name.to_string()),
        _ => None,
    }
}

pub(crate) fn array_elements<'a>(
    expression: &'a Expression<'a>,
) -> Option<Vec<&'a Expression<'a>>> {
    let Expression::ArrayExpression(array) = expression else {
        return None;
    };
    let mut elements = Vec::new();
    for element in &array.elements {
        match element {
            ArrayExpressionElement::Elision(_) | ArrayExpressionElement::SpreadElement(_) => {
                return None;
            }
            _ => elements.push(element.as_expression()?),
        }
    }
    Some(elements)
}

pub(crate) fn call_arguments<'a>(call: &'a CallExpression<'a>) -> Vec<&'a Expression<'a>> {
    call.arguments
        .iter()
        .filter_map(Argument::as_expression)
        .collect()
}

pub(crate) fn new_arguments<'a>(call: &'a NewExpression<'a>) -> Vec<&'a Expression<'a>> {
    call.arguments
        .iter()
        .filter_map(Argument::as_expression)
        .collect()
}

pub(crate) fn static_member_name<'a>(expression: &'a Expression<'a>) -> Option<&'a str> {
    match expression {
        Expression::StaticMemberExpression(member) => Some(member.property.name.as_str()),
        Expression::ComputedMemberExpression(member) => match &member.expression {
            Expression::StringLiteral(literal) => Some(literal.value.as_str()),
            _ => None,
        },
        _ => None,
    }
}

/// Parameter slot `index` of the function or arrow node `owner`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ParameterSlot {
    pub owner: oxc_semantic::NodeId,
    pub index: u32,
}

/// The one parameter-slot resolver, shared by the scanner's [`Model`], the
/// naming recognizers and the purpose finders. It needs only the parse and its
/// scope analysis, so a caller without a [`Model`] builds it directly.
///
/// Under it a function's Nth plain, never-written formal and its own
/// `arguments[N]` are the same slot, whichever representation the capture
/// uses.
pub(crate) struct ParameterSlots<'s, 'a> {
    scoping: &'s Scoping,
    nodes: &'s AstNodes<'a>,
    facts: FormalSlotFacts,
}

impl<'s, 'a> ParameterSlots<'s, 'a> {
    /// Collect the per-function slot facts of one parse.
    pub(crate) fn new(
        program: &Program<'_>,
        scoping: &'s Scoping,
        nodes: &'s AstNodes<'a>,
    ) -> Self {
        Self {
            scoping,
            nodes,
            facts: FormalSlotFacts::collect(program, scoping),
        }
    }

    /// The parameter slot an expression denotes, in either capture
    /// representation:
    ///
    /// * `arguments[k]` with an unbound `arguments` and a numeric literal
    ///   index is slot `k` of the nearest enclosing ordinary function (an
    ///   arrow has no `arguments` of its own) that never rebinds
    ///   `arguments` itself;
    /// * an identifier that resolves, by scope, to the `k`th formal of a
    ///   function or arrow is slot `k` of that function, subject to
    ///   [`Self::formal_symbol_slot`].
    ///
    /// The slot names the owning function. A caller matching one function
    /// must compare the owner with it.
    pub(crate) fn parameter_slot(&self, expression: &Expression<'_>) -> Option<ParameterSlot> {
        match expression.without_parentheses() {
            Expression::ComputedMemberExpression(member) => {
                let Expression::Identifier(identifier) = member.object.without_parentheses() else {
                    return None;
                };
                if identifier.name != "arguments"
                    || crate::analysis::identifier_symbol(identifier, self.scoping).is_some()
                {
                    return None;
                }
                let index = numeric_index(&member.expression)?;
                let reference = identifier.reference_id.get()?;
                let node = self.scoping.get_reference(reference).node_id();
                let owner = self.arguments_owner(node)?;
                if self.facts.arguments_rebound.contains(&owner) {
                    return None;
                }
                Some(ParameterSlot { owner, index })
            }
            Expression::Identifier(identifier) => self.formal_symbol_slot(
                crate::analysis::identifier_symbol(identifier, self.scoping)?,
            ),
            _ => None,
        }
    }

    /// [`Self::parameter_slot`] for a value that must still be the incoming
    /// argument where it is read: an `arguments[k]` read additionally fails
    /// when its owner writes or deletes a member of its own `arguments`
    /// object, since the read may then see the written value. A formal slot
    /// already refuses that owner.
    pub(crate) fn incoming_slot(&self, expression: &Expression<'_>) -> Option<ParameterSlot> {
        let slot = self.parameter_slot(expression)?;
        let is_arguments = matches!(
            expression.without_parentheses(),
            Expression::ComputedMemberExpression(_)
        );
        if is_arguments && self.facts.arguments_written.contains(&slot.owner) {
            return None;
        }
        Some(slot)
    }

    /// The parameter slot a named formal stands for. A formal is equivalent
    /// to its argument position only when nothing can make the binding differ
    /// from the call argument:
    ///
    /// * it is a plain `BindingIdentifier` item: destructured, rest and
    ///   defaulted parameters are refused (a default replaces an `undefined`
    ///   argument, so the binding is not the argument);
    /// * the binding is never written and never redeclared in the body;
    /// * an ordinary owner never writes, deletes, aliases or passes its own
    ///   `arguments` object (a sloppy-mode mapped `arguments` write changes
    ///   the formal). Calling `arguments[k](...)` is accepted: it is the
    ///   collector ABI's scheduler call, although the callee receives the
    ///   object as `this`;
    /// * no direct `eval` or `with` can rebind the formal dynamically.
    pub(crate) fn formal_symbol_slot(&self, symbol: SymbolId) -> Option<ParameterSlot> {
        let (owner, index) = formal_parameter_slot(self.nodes, self.scoping, symbol)?;
        let params = match self.nodes.kind(owner) {
            AstKind::Function(function) => &function.params,
            AstKind::ArrowFunctionExpression(arrow) => &arrow.params,
            _ => return None,
        };
        let param = params.items.get(index)?;
        if param.initializer.is_some()
            || !matches!(param.pattern, BindingPattern::BindingIdentifier(_))
        {
            return None;
        }
        if self.scoping.symbol_is_mutated(symbol)
            || !self.scoping.symbol_redeclarations(symbol).is_empty()
        {
            return None;
        }
        if self.facts.dynamic_scope.contains(&owner) || self.facts.arguments_escape.contains(&owner)
        {
            return None;
        }
        Some(ParameterSlot {
            owner,
            index: u32::try_from(index).ok()?,
        })
    }

    /// The nearest ordinary function enclosing a node: the owner of the
    /// `arguments` object read there.
    fn arguments_owner(&self, node_id: oxc_semantic::NodeId) -> Option<oxc_semantic::NodeId> {
        let mut current = node_id;
        for _ in 0..512 {
            let parent = self.nodes.parent_id(current);
            if parent == current {
                return None;
            }
            current = parent;
            if matches!(self.nodes.kind(current), AstKind::Function(_)) {
                return Some(current);
            }
        }
        None
    }
}

/// Per-function facts that decide whether a named formal may stand for its
/// argument position. Collected once per parse.
#[derive(Default)]
struct FormalSlotFacts {
    /// Ordinary functions whose own `arguments` object is used other than as
    /// a plain `arguments[k]` or `arguments.length` read.
    arguments_escape: HashSet<oxc_semantic::NodeId>,
    /// Ordinary functions that rebind their `arguments` name itself
    /// (`arguments = ...`, sloppy mode). Their `arguments[k]` no longer
    /// denotes a parameter slot.
    arguments_rebound: HashSet<oxc_semantic::NodeId>,
    /// Ordinary functions that write, update or delete a member of their own
    /// `arguments` object. A subset of `arguments_escape`.
    arguments_written: HashSet<oxc_semantic::NodeId>,
    /// Functions and arrows lexically enclosing a direct `eval` or `with`.
    dynamic_scope: HashSet<oxc_semantic::NodeId>,
}

impl FormalSlotFacts {
    fn collect(program: &Program<'_>, scoping: &Scoping) -> Self {
        let mut collector = FormalSlotCollector {
            scoping,
            functions: Vec::new(),
            plain_reads: HashSet::new(),
            facts: FormalSlotFacts::default(),
        };
        collector.visit_program(program);
        collector.facts
    }
}

struct FormalSlotCollector<'s> {
    scoping: &'s Scoping,
    /// Enclosing function/arrow nodes, innermost last; `true` for an arrow.
    functions: Vec<(oxc_semantic::NodeId, bool)>,
    /// `arguments` identifier spans used as a plain indexed/length read.
    plain_reads: HashSet<u32>,
    facts: FormalSlotFacts,
}

impl FormalSlotCollector<'_> {
    fn is_arguments(&self, expression: &Expression<'_>) -> bool {
        matches!(expression.without_parentheses(), Expression::Identifier(identifier)
            if identifier.name == "arguments"
                && crate::analysis::identifier_symbol(identifier, self.scoping).is_none())
    }

    fn arguments_member(&self, expression: &Expression<'_>) -> bool {
        match expression.without_parentheses() {
            Expression::ComputedMemberExpression(member) => self.is_arguments(&member.object),
            Expression::StaticMemberExpression(member) => self.is_arguments(&member.object),
            _ => false,
        }
    }

    fn arguments_owner(&self) -> Option<oxc_semantic::NodeId> {
        self.functions
            .iter()
            .rev()
            .find(|(_, arrow)| !arrow)
            .map(|(owner, _)| *owner)
    }

    fn mark_arguments_owner(&mut self) {
        if let Some(owner) = self.arguments_owner() {
            self.facts.arguments_escape.insert(owner);
        }
    }

    fn mark_arguments_written(&mut self) {
        if let Some(owner) = self.arguments_owner() {
            self.facts.arguments_escape.insert(owner);
            self.facts.arguments_written.insert(owner);
        }
    }

    fn mark_dynamic_scope(&mut self) {
        let enclosing: Vec<_> = self.functions.iter().map(|(node, _)| *node).collect();
        self.facts.dynamic_scope.extend(enclosing);
    }
}

impl<'a> Visit<'a> for FormalSlotCollector<'_> {
    fn visit_function(&mut self, function: &Function<'a>, flags: oxc_syntax::scope::ScopeFlags) {
        self.functions.push((function.node_id.get(), false));
        walk::walk_function(self, function, flags);
        self.functions.pop();
    }

    fn visit_arrow_function_expression(&mut self, arrow: &ArrowFunctionExpression<'a>) {
        self.functions.push((arrow.node_id.get(), true));
        walk::walk_arrow_function_expression(self, arrow);
        self.functions.pop();
    }

    fn visit_computed_member_expression(&mut self, member: &ComputedMemberExpression<'a>) {
        if self.is_arguments(&member.object) && numeric_index(&member.expression).is_some() {
            self.plain_reads.insert(member.object.span().start);
        }
        walk::walk_computed_member_expression(self, member);
    }

    fn visit_static_member_expression(&mut self, member: &StaticMemberExpression<'a>) {
        if self.is_arguments(&member.object) && member.property.name == "length" {
            self.plain_reads.insert(member.object.span().start);
        }
        walk::walk_static_member_expression(self, member);
    }

    fn visit_simple_assignment_target(&mut self, target: &SimpleAssignmentTarget<'a>) {
        // Assignment, update and for-in/of targets all reach this visit.
        let written = match target {
            SimpleAssignmentTarget::ComputedMemberExpression(member) => {
                self.is_arguments(&member.object)
            }
            SimpleAssignmentTarget::StaticMemberExpression(member) => {
                self.is_arguments(&member.object)
            }
            _ => false,
        };
        if written {
            self.mark_arguments_written();
        }
        walk::walk_simple_assignment_target(self, target);
    }

    fn visit_unary_expression(&mut self, unary: &UnaryExpression<'a>) {
        if unary.operator == oxc_syntax::operator::UnaryOperator::Delete
            && self.arguments_member(&unary.argument)
        {
            self.mark_arguments_written();
        }
        walk::walk_unary_expression(self, unary);
    }

    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if matches!(call.callee.without_parentheses(), Expression::Identifier(identifier)
            if identifier.name == "eval"
                && crate::analysis::identifier_symbol(identifier, self.scoping).is_none())
        {
            self.mark_dynamic_scope();
        }
        walk::walk_call_expression(self, call);
    }

    fn visit_with_statement(&mut self, with: &WithStatement<'a>) {
        self.mark_dynamic_scope();
        walk::walk_with_statement(self, with);
    }

    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        // Any other use (alias, call argument, spread, `arguments.callee`)
        // lets code outside this function see or change the formals.
        if identifier.name == "arguments"
            && crate::analysis::identifier_symbol(identifier, self.scoping).is_none()
            && !self.plain_reads.contains(&identifier.span.start)
        {
            self.mark_arguments_owner();
            let written = identifier
                .reference_id
                .get()
                .is_some_and(|reference| self.scoping.get_reference(reference).is_write());
            if written {
                if let Some(owner) = self.arguments_owner() {
                    self.facts.arguments_rebound.insert(owner);
                }
            }
        }
    }
}

pub(crate) fn numeric_index(expression: &Expression<'_>) -> Option<u32> {
    match expression {
        Expression::NumericLiteral(literal)
            if literal.value.fract() == 0.0
                && literal.value >= 0.0
                && literal.value <= u32::MAX as f64 =>
        {
            Some(literal.value as u32)
        }
        _ => None,
    }
}

pub(crate) fn numeric_value(expression: &Expression<'_>) -> Option<f64> {
    match expression {
        Expression::NumericLiteral(literal) => Some(literal.value),
        Expression::UnaryExpression(unary) => match unary.operator {
            oxc_syntax::operator::UnaryOperator::UnaryNegation => {
                Some(-numeric_value(&unary.argument)?)
            }
            oxc_syntax::operator::UnaryOperator::UnaryPlus => numeric_value(&unary.argument),
            _ => None,
        },
        _ => None,
    }
}

const MAX_LITERAL_DEPTH: usize = 8;
const MAX_LITERAL_NODES: usize = 512;

/// Recursively source-closed literal value for an expression: primitives as
/// before, plus bounded array/object literals and array holes. The whole
/// expression must be closed under literals; a single dynamic element, spread,
/// computed key or method makes the result `None` so the caller keeps the exact
/// source text instead of a partial or fabricated value. No JavaScript is
/// executed and no runtime value is invented.
pub(crate) fn literal_value_recursive(expression: &Expression<'_>) -> Option<LiteralValue> {
    let mut budget = MAX_LITERAL_NODES;
    literal_value_at(expression, 0, &mut budget)
}

fn literal_value_at(
    expression: &Expression<'_>,
    depth: usize,
    budget: &mut usize,
) -> Option<LiteralValue> {
    if depth > MAX_LITERAL_DEPTH || *budget == 0 {
        return None;
    }
    *budget -= 1;
    match expression {
        Expression::ArrayExpression(array) => {
            let mut values = Vec::with_capacity(array.elements.len());
            for element in &array.elements {
                match element {
                    ArrayExpressionElement::Elision(_) => values.push(LiteralValue::Hole),
                    ArrayExpressionElement::SpreadElement(_) => return None,
                    _ => {
                        let expression = element.as_expression()?;
                        values.push(literal_value_at(expression, depth + 1, budget)?);
                    }
                }
            }
            Some(LiteralValue::Array(values))
        }
        Expression::ObjectExpression(object) => {
            let mut values = std::collections::BTreeMap::new();
            for property in &object.properties {
                let ObjectPropertyKind::ObjectProperty(property) = property else {
                    return None;
                };
                if property.computed || property.method || property.kind != PropertyKind::Init {
                    return None;
                }
                let name = property_key_name(&property.key)?;
                let value = literal_value_at(&property.value, depth + 1, budget)?;
                values.insert(name, value);
            }
            Some(LiteralValue::Object(values))
        }
        _ => literal_value(expression),
    }
}

pub(crate) fn literal_value(expression: &Expression<'_>) -> Option<LiteralValue> {
    match expression {
        Expression::StringLiteral(literal) => Some(LiteralValue::String(literal.value.to_string())),
        Expression::NumericLiteral(literal) => Some(LiteralValue::Number(literal.value)),
        Expression::BooleanLiteral(literal) => Some(LiteralValue::Boolean(literal.value)),
        Expression::NullLiteral(_) => Some(LiteralValue::Null),
        Expression::UnaryExpression(unary)
            if unary.operator == oxc_syntax::operator::UnaryOperator::Void =>
        {
            Some(LiteralValue::Undefined)
        }
        Expression::Identifier(identifier) if identifier.name == "undefined" => {
            Some(LiteralValue::Undefined)
        }
        _ => None,
    }
}

pub(crate) fn value_kind(expression: &Expression<'_>) -> ValueKind {
    match expression {
        Expression::ArrayExpression(_) => ValueKind::Array,
        Expression::ObjectExpression(_) => ValueKind::Object,
        Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_) => {
            ValueKind::Function
        }
        Expression::Identifier(_) => ValueKind::Identifier,
        Expression::CallExpression(_) | Expression::NewExpression(_) => ValueKind::Call,
        _ if literal_value(expression).is_some() => ValueKind::Literal,
        _ => ValueKind::Other,
    }
}

pub(crate) fn is_integer_array(expression: &Expression<'_>) -> Option<Vec<usize>> {
    let elements = array_elements(expression)?;
    let mut values = Vec::with_capacity(elements.len());
    for element in elements {
        let value = numeric_index(element)?;
        values.push(value as usize);
    }
    Some(values)
}

pub(crate) fn is_boolean_array(expression: &Expression<'_>) -> Option<Vec<bool>> {
    let elements = array_elements(expression)?;
    let mut values = Vec::with_capacity(elements.len());
    for element in elements {
        match element {
            Expression::BooleanLiteral(literal) => values.push(literal.value),
            _ => return None,
        }
    }
    Some(values)
}

fn format_number(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e18 {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every function site of `source` keyed by its exact source text, with the
    /// name of the binding the site is held under.
    fn site_bindings(source: &str) -> Vec<(String, Option<String>)> {
        let allocator = oxc_allocator::Allocator::default();
        let parsed = crate::analysis::parse(&allocator, source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let (scoping, nodes) = crate::analysis::analyze(&parsed.program);
        let model = Model::new(
            source,
            &parsed.program,
            &scoping,
            &nodes,
            ProgramPropertiesLimits::default(),
        )
        .expect("model");
        model
            .all_sites()
            .iter()
            .map(|site| {
                (
                    source[site.span.start as usize..site.span.end as usize].to_string(),
                    site.binding_symbol
                        .map(|symbol| model.scoping().symbol_name(symbol).to_string()),
                )
            })
            .collect()
    }

    fn binding_of<'a>(sites: &'a [(String, Option<String>)], text: &str) -> Option<&'a str> {
        sites
            .iter()
            .find(|(source, _)| source == text)
            .unwrap_or_else(|| panic!("site {text}"))
            .1
            .as_deref()
    }

    #[test]
    fn declarator_initializer_function_is_held_by_its_binding() {
        let sites = site_bindings(
            "function m(){ let f = () => {}; let g = function () {}; let h = wrap(function () { return 1 }); let k = [() => 2]; return [f, g, h, k]; } m();",
        );
        assert_eq!(binding_of(&sites, "() => {}"), Some("f"));
        assert_eq!(binding_of(&sites, "function () {}"), Some("g"));
        // Nested inside a larger initializer: the declarator does not hold it.
        assert_eq!(binding_of(&sites, "function () { return 1 }"), None);
        assert_eq!(binding_of(&sites, "() => 2"), None);
    }

    #[test]
    fn named_function_keeps_its_own_identifier_binding() {
        let sites =
            site_bindings("function m(){ let a = function b(){ return b }; return a; } m();");
        assert_eq!(binding_of(&sites, "function b(){ return b }"), Some("b"));
    }

    #[test]
    fn assigned_values_keep_scope_last_direct_write_and_initializer_precedence() {
        let source = "function main(){ let x; x=1; {let x; x=99;} x+=2; x=7; let y; ({y}=input); let z=0; z=4; return [x,y,z]; } main();";
        let allocator = oxc_allocator::Allocator::default();
        let parsed = crate::analysis::parse(&allocator, source);
        let (scoping, nodes) = crate::analysis::analyze(&parsed.program);
        let model = Model::new(
            source,
            &parsed.program,
            &scoping,
            &nodes,
            ProgramPropertiesLimits::default(),
        )
        .unwrap();
        let symbol = |name| {
            *model
                .main_symbols
                .iter()
                .find(|s| scoping.symbol_name(**s) == name)
                .unwrap()
        };
        assert_eq!(
            model
                .symbol_assigned_value(symbol("x"))
                .map(|e| model.exact(e.span())),
            Some("7")
        );
        assert!(model.symbol_assigned_value(symbol("y")).is_none());
        assert_eq!(
            model
                .symbol_assigned_value(symbol("z"))
                .map(|e| model.exact(e.span())),
            Some("4")
        );
        assert_eq!(
            model
                .symbol_value(symbol("z"))
                .map(|e| model.exact(e.span())),
            Some("0")
        );
    }
}

/// The global dotted path of an unshadowed identifier / static member chain
/// (`Object.defineProperty` -> `["Object", "defineProperty"]`); empty otherwise.
pub(crate) fn expression_path(model: &Model<'_, '_>, e: &Expression<'_>) -> Vec<String> {
    match e {
        Expression::Identifier(i)
            if crate::analysis::identifier_symbol(i, model.scoping).is_none() =>
        {
            vec![i.name.to_string()]
        }
        Expression::StaticMemberExpression(m) => {
            let mut p = expression_path(model, &m.object);
            if !p.is_empty() {
                p.push(m.property.name.to_string());
            }
            p
        }
        _ => Vec::new(),
    }
}
