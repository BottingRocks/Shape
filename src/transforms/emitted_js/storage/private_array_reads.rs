//! Replace snapshots of private own data cells while that cell is unchanged.
//! Arrays must come from dense literals or guarded native slice copies and
//! never escape, become call receivers, or have captured/structural accesses.
//! Property reads are rematerialized; arithmetic/coercions are never duplicated.
use super::super::{adjacent_ssa::LocalReads, index::SpanIndex};
use crate::analysis;
use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstKind};
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::scope::ScopeFlags;
use std::collections::{BTreeMap, BTreeSet};
use crate::transforms::emitted_js::support::unwrap;

fn index(e: &Expression<'_>) -> Option<usize> {
    match unwrap(e) {
        Expression::NumericLiteral(n)
            if n.value >= 0.0 && n.value < 65536.0 && n.value.fract() == 0.0 =>
        {
            Some(n.value as usize)
        }
        _ => None,
    }
}
fn cell(e: &Expression<'_>, scoping: &Scoping) -> Option<(SymbolId, usize)> {
    let Expression::ComputedMemberExpression(m) = unwrap(e) else {
        return None;
    };
    let Expression::Identifier(id) = unwrap(&m.object) else {
        return None;
    };
    if m.optional {
        return None;
    }
    Some((
        analysis::identifier_symbol(id, scoping)?,
        index(&m.expression)?,
    ))
}
fn member<'s, 'a>(e: &'s Expression<'a>) -> Option<(&'s IdentifierReference<'a>, &'s str)> {
    let (object, key) = match unwrap(e) {
        Expression::StaticMemberExpression(m) if !m.optional => {
            (&m.object, m.property.name.as_str())
        }
        Expression::ComputedMemberExpression(m) if !m.optional => match unwrap(&m.expression) {
            Expression::StringLiteral(k) => (&m.object, k.value.as_str()),
            _ => return None,
        },
        _ => return None,
    };
    match unwrap(object) {
        Expression::Identifier(id) => Some((id, key)),
        _ => None,
    }
}
#[derive(Clone, Copy)]
enum Origin {
    Literal(usize),
    Slice(SymbolId, Span),
}
#[derive(Clone, Copy)]
struct Store {
    array: SymbolId,
    index: usize,
    span: Span,
}
#[derive(Default)]
struct Facts {
    owners: SpanIndex,
    declarations: BTreeMap<SymbolId, Span>,
    stores: Vec<Store>,
    targets: BTreeSet<Span>,
    safe_targets: BTreeSet<Span>,
    bare: BTreeSet<Span>,
    nonvalues: BTreeSet<Span>,
    dynamic: bool,
    pending: bool,
    directive_barrier: bool,
}
impl Facts {
    fn body(&mut self, body: &[Statement<'_>]) {
        for s in body {
            if let Statement::VariableDeclaration(d) = s {
                if !matches!(
                    d.kind,
                    VariableDeclarationKind::Let | VariableDeclarationKind::Const
                ) {
                    continue;
                }
                for item in &d.declarations {
                    if let BindingPattern::BindingIdentifier(id) = &item.id {
                        if let Some(symbol) = id.symbol_id.get() {
                            self.declarations.insert(symbol, d.span);
                        }
                    }
                }
            }
        }
    }
}
struct Collect<'s> {
    facts: Facts,
    scoping: &'s Scoping,
}
impl<'a> Visit<'a> for Collect<'_> {
    fn visit_expression_statement(&mut self, s: &ExpressionStatement<'a>) {
        self.facts.directive_barrier |= matches!(s.expression, Expression::StringLiteral(_));
        walk::walk_expression_statement(self, s);
    }
    fn visit_function(&mut self, f: &Function<'a>, flags: ScopeFlags) {
        self.facts.owners.push(f.span);
        self.facts.pending |=
            f.id.as_ref()
                .is_some_and(|id| id.name.starts_with("$THREAD_"))
                && f.body
                    .as_ref()
                    .is_some_and(|b| b.statements.is_empty() && b.directives.is_empty());
        walk::walk_function(self, f, flags);
    }
    fn visit_arrow_function_expression(&mut self, f: &ArrowFunctionExpression<'a>) {
        self.facts.owners.push(f.span);
        walk::walk_arrow_function_expression(self, f);
    }
    fn visit_class(&mut self, c: &Class<'a>) {
        self.facts.owners.push(c.span);
        walk::walk_class(self, c);
    }
    fn visit_function_body(&mut self, b: &FunctionBody<'a>) {
        self.facts.body(&b.statements);
        walk::walk_function_body(self, b);
    }
    fn visit_block_statement(&mut self, b: &BlockStatement<'a>) {
        self.facts.body(&b.body);
        walk::walk_block_statement(self, b);
    }
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        self.facts.dynamic |= id.name == "eval";
    }
    fn visit_with_statement(&mut self, s: &WithStatement<'a>) {
        self.facts.dynamic = true;
        walk::walk_with_statement(self, s);
    }
    fn visit_assignment_target(&mut self, t: &AssignmentTarget<'a>) {
        match t {
            AssignmentTarget::ComputedMemberExpression(m) => {
                self.facts.targets.insert(m.span);
            }
            AssignmentTarget::StaticMemberExpression(m) => {
                self.facts.targets.insert(m.span);
            }
            _ => {}
        }
        walk::walk_assignment_target(self, t);
    }
    fn visit_simple_assignment_target(&mut self, t: &SimpleAssignmentTarget<'a>) {
        match t {
            SimpleAssignmentTarget::ComputedMemberExpression(m) => {
                self.facts.targets.insert(m.span);
            }
            SimpleAssignmentTarget::StaticMemberExpression(m) => {
                self.facts.targets.insert(m.span);
            }
            _ => {}
        }
        walk::walk_simple_assignment_target(self, t);
    }
    fn visit_assignment_expression(&mut self, a: &AssignmentExpression<'a>) {
        if let AssignmentTarget::ComputedMemberExpression(m) = &a.left {
            if let Expression::Identifier(id) = unwrap(&m.object) {
                if let (Some(array), Some(index)) = (
                    analysis::identifier_symbol(id, self.scoping),
                    index(&m.expression),
                ) {
                    self.facts.safe_targets.insert(m.span);
                    self.facts.stores.push(Store {
                        array,
                        index,
                        span: a.span,
                    });
                }
            }
        }
        walk::walk_assignment_expression(self, a);
    }
    fn visit_update_expression(&mut self, u: &UpdateExpression<'a>) {
        if let SimpleAssignmentTarget::ComputedMemberExpression(m) = &u.argument {
            if let Expression::Identifier(id) = unwrap(&m.object) {
                if let (Some(array), Some(index)) = (
                    analysis::identifier_symbol(id, self.scoping),
                    index(&m.expression),
                ) {
                    self.facts.safe_targets.insert(m.span);
                    self.facts.stores.push(Store {
                        array,
                        index,
                        span: u.span,
                    });
                }
            }
        }
        walk::walk_update_expression(self, u);
    }
    fn visit_call_expression(&mut self, c: &CallExpression<'a>) {
        self.facts.bare.insert(unwrap(&c.callee).span());
        walk::walk_call_expression(self, c);
    }
    fn visit_tagged_template_expression(&mut self, t: &TaggedTemplateExpression<'a>) {
        self.facts.bare.insert(unwrap(&t.tag).span());
        walk::walk_tagged_template_expression(self, t);
    }
    fn visit_object_property(&mut self, p: &ObjectProperty<'a>) {
        if p.shorthand {
            self.facts.nonvalues.insert(p.value.span());
        }
        walk::walk_object_property(self, p);
    }
    fn visit_unary_expression(&mut self, u: &UnaryExpression<'a>) {
        if u.operator == UnaryOperator::Delete {
            self.facts.nonvalues.insert(unwrap(&u.argument).span());
        }
        walk::walk_unary_expression(self, u);
    }
}

pub(crate) fn inline_private_array_reads(
    source: &str,
    protected: &BTreeSet<String>,
    checked_captures: bool,
) -> Result<(String, usize), String> {
    // Most small threads contain neither arrays nor indexed reads.
    if !source.contains('[') {
        return Ok((source.into(), 0));
    }
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut collect = Collect {
        facts: Facts::default(),
        scoping: &scoping,
    };
    collect.visit_program(&program);
    let mut facts = collect.facts;
    if facts.dynamic || facts.pending && !checked_captures {
        return Ok((source.into(), 0));
    }
    facts.owners.build();
    let ordinary = super::super::dense_arrays::ordinary_prototypes(&program, &scoping);
    let mut locals = LocalReads::default();
    locals.visit_program(&program);
    locals.build(&scoping, &nodes, protected);
    let mut scan = Scan {
        source,
        scoping: &scoping,
        nodes: &nodes,
        facts: &facts,
        locals: &locals,
        protected,
        ordinary,
        arrays: BTreeMap::new(),
        edits: vec![],
        count: 0,
    };
    for (&symbol, &declaration) in &facts.declarations {
        if protected.contains(scoping.symbol_name(symbol))
            || !scoping.symbol_redeclarations(symbol).is_empty()
            || scoping
                .get_resolved_references(symbol)
                .any(|r| r.is_write())
        {
            continue;
        }
        let AstKind::VariableDeclarator(d) = nodes.kind(scoping.symbol_declaration(symbol)) else {
            continue;
        };
        let Some(init) = &d.init else {
            continue;
        };
        let origin = match unwrap(init) {
            Expression::ArrayExpression(a)
                if a.elements.iter().all(|e| e.as_expression().is_some()) =>
            {
                Some(Origin::Literal(a.elements.len()))
            }
            Expression::CallExpression(c) if ordinary && !c.optional && c.arguments.is_empty() => {
                member(&c.callee)
                    .and_then(|(id, name)| {
                        (name == "slice")
                            .then(|| analysis::identifier_symbol(id, &scoping))
                            .flatten()
                    })
                    .map(|s| Origin::Slice(s, init.span()))
            }
            _ => None,
        };
        if let Some(origin) = origin {
            if scan.closed(symbol, declaration) {
                scan.arrays.insert(symbol, origin);
            }
        }
    }
    // An out-of-bounds write can reach an inherited setter and expose the
    // array as `this`. Only writes to existing own data cells stay private.
    let exposed: Vec<_> = facts
        .stores
        .iter()
        .filter(|w| !scan.own_at(w.array, w.index, w.span, 16))
        .map(|w| w.array)
        .collect();
    for symbol in exposed {
        scan.arrays.remove(&symbol);
    }
    super::super::support::for_each_list(&program, |list| scan.body(list));
    let applied = analysis::apply_edits(source, scan.edits);
    if !applied.rejected.is_empty() {
        return Err("private array snapshot edits overlap".into());
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output)?;
    Ok((applied.output, scan.count))
}

struct Scan<'s, 'a> {
    source: &'s str,
    scoping: &'s Scoping,
    nodes: &'s AstNodes<'a>,
    facts: &'s Facts,
    locals: &'s LocalReads,
    protected: &'s BTreeSet<String>,
    ordinary: bool,
    arrays: BTreeMap<SymbolId, Origin>,
    edits: Vec<analysis::Edit>,
    count: usize,
}
impl Scan<'_, '_> {
    fn literal_length(&self, symbol: SymbolId) -> Option<usize> {
        let AstKind::VariableDeclarator(d) =
            self.nodes.kind(self.scoping.symbol_declaration(symbol))
        else {
            return None;
        };
        let Expression::ArrayExpression(a) = unwrap(d.init.as_ref()?) else {
            return None;
        };
        a.elements
            .iter()
            .all(|e| e.as_expression().is_some())
            .then_some(a.elements.len())
    }
    fn fixed_bound(
        &self,
        e: &Expression<'_>,
        array: SymbolId,
        len: usize,
        depth: usize,
    ) -> Option<f64> {
        if depth == 0 {
            return None;
        }
        match unwrap(e) {
            Expression::NumericLiteral(n) => Some(n.value),
            Expression::Identifier(id) => {
                let s = analysis::identifier_symbol(id, self.scoping)?;
                if !self.locals.initialized(s, id.span)
                    || !self.scoping.symbol_redeclarations(s).is_empty()
                    || self
                        .scoping
                        .get_resolved_references(s)
                        .any(|r| r.is_write())
                {
                    return None;
                }
                let AstKind::VariableDeclarator(d) =
                    self.nodes.kind(self.scoping.symbol_declaration(s))
                else {
                    return None;
                };
                self.fixed_bound(d.init.as_ref()?, array, len, depth - 1)
            }
            Expression::BinaryExpression(b)
                if matches!(
                    b.operator,
                    BinaryOperator::Addition | BinaryOperator::Subtraction
                ) =>
            {
                let left = self.fixed_bound(&b.left, array, len, depth - 1)?;
                let right = self.fixed_bound(&b.right, array, len, depth - 1)?;
                Some(if b.operator == BinaryOperator::Addition {
                    left + right
                } else {
                    left - right
                })
            }
            other => member(other).and_then(|(id, key)| {
                (key == "length" && analysis::identifier_symbol(id, self.scoping) == Some(array))
                    .then_some(len as f64)
            }),
        }
    }
    fn bounded_counter(&self, array: SymbolId, e: &Expression<'_>, at: Span) -> Option<SymbolId> {
        let Expression::Identifier(id) = unwrap(e) else {
            return None;
        };
        let counter = analysis::identifier_symbol(id, self.scoping)?;
        let len = self.literal_length(array)?;
        if !self.locals.initialized(array, at)
            || !self.scoping.symbol_redeclarations(counter).is_empty()
        {
            return None;
        }
        for node in self.nodes.iter() {
            let AstKind::ForStatement(loop_) = node.kind() else {
                continue;
            };
            if !(loop_.body.span().start <= at.start && at.end <= loop_.body.span().end) {
                continue;
            }
            let Some(ForStatementInit::VariableDeclaration(d)) = &loop_.init else {
                continue;
            };
            if d.kind != VariableDeclarationKind::Let {
                continue;
            }
            if !d.declarations.iter().any(|item| matches!(&item.id,BindingPattern::BindingIdentifier(id)if id.symbol_id.get()==Some(counter))
                && matches!(item.init.as_ref().map(unwrap),Some(Expression::NumericLiteral(n))if n.value==0.0)) {continue;}
            let Some(update) = &loop_.update else {
                continue;
            };
            let same = |e: &Expression<'_>| matches!(unwrap(e),Expression::Identifier(id)if analysis::identifier_symbol(id,self.scoping)==Some(counter));
            let step = match unwrap(update) {
                Expression::UpdateExpression(u) => {
                    u.operator == UpdateOperator::Increment
                        && matches!(&u.argument,SimpleAssignmentTarget::AssignmentTargetIdentifier(id)if analysis::identifier_symbol(id,self.scoping)==Some(counter))
                }
                Expression::AssignmentExpression(a) if matches!(&a.left,AssignmentTarget::AssignmentTargetIdentifier(id)if analysis::identifier_symbol(id,self.scoping)==Some(counter)) => {
                    a.operator == AssignmentOperator::Addition
                        && matches!(unwrap(&a.right),Expression::NumericLiteral(n)if n.value==1.0)
                        || a.operator == AssignmentOperator::Assign
                            && matches!(unwrap(&a.right),Expression::BinaryExpression(b)if b.operator==BinaryOperator::Addition&&same(&b.left)&&matches!(unwrap(&b.right),Expression::NumericLiteral(n)if n.value==1.0))
                }
                _ => false,
            };
            if !step
                || self.scoping.get_resolved_references(counter).any(|r| {
                    r.is_write() && {
                        let w = self.nodes.get_node(r.node_id()).kind().span();
                        !(update.span().start <= w.start && w.end <= update.span().end)
                    }
                })
            {
                continue;
            }
            let Some(Expression::BinaryExpression(test)) = loop_.test.as_ref().map(unwrap) else {
                continue;
            };
            let upper = if same(&test.left) {
                let bound = self.fixed_bound(&test.right, array, len, 8);
                match test.operator {
                    BinaryOperator::LessThan
                    | BinaryOperator::Inequality
                    | BinaryOperator::StrictInequality => bound,
                    BinaryOperator::LessEqualThan => bound.map(|n| n + 1.0),
                    _ => None,
                }
            } else if same(&test.right) {
                let bound = self.fixed_bound(&test.left, array, len, 8);
                match test.operator {
                    BinaryOperator::GreaterThan
                    | BinaryOperator::Inequality
                    | BinaryOperator::StrictInequality => bound,
                    BinaryOperator::GreaterEqualThan => bound.map(|n| n + 1.0),
                    _ => None,
                }
            } else {
                None
            };
            if upper
                .is_some_and(|n| n.is_finite() && n >= 0.0 && n.fract() == 0.0 && n <= len as f64)
            {
                return Some(counter);
            }
        }
        None
    }
    // Peel only wrappers that preserve a Reference. A method call/tag would
    // hand the array to arbitrary code as `this`, even if no alias is stored.
    fn referenced_as_receiver(&self, mut node: oxc_syntax::node::NodeId, span: Span) -> bool {
        loop {
            match self.nodes.parent_kind(node) {
                AstKind::ParenthesizedExpression(_) | AstKind::ChainExpression(_) => {
                    node = self.nodes.parent_id(node)
                }
                AstKind::CallExpression(c) => {
                    return unwrap(&c.callee).span() == span
                        || c.callee.span() == self.nodes.get_node(node).kind().span()
                }
                AstKind::TaggedTemplateExpression(t) => {
                    return unwrap(&t.tag).span() == span
                        || t.tag.span() == self.nodes.get_node(node).kind().span()
                }
                _ => return false,
            }
        }
    }
    fn closed(&self, symbol: SymbolId, declaration: Span) -> bool {
        self.scoping.get_resolved_references(symbol).all(|r|{
            let at=self.nodes.get_node(r.node_id()).kind().span();
            if at.start<declaration.end||self.facts.owners.enclosing(at)!=self.facts.owners.enclosing(declaration){return false;}
            let parent=self.nodes.parent_id(r.node_id());
            match self.nodes.get_node(parent).kind(){
                AstKind::ComputedMemberExpression(m)if !m.optional&&m.object.span()==at&&index(&m.expression).is_some()=>{
                    !self.facts.nonvalues.contains(&m.span)&&!self.referenced_as_receiver(parent,m.span)
                        &&(!self.facts.targets.contains(&m.span)||self.facts.safe_targets.contains(&m.span))
                }
                AstKind::ComputedMemberExpression(m)if !m.optional&&m.object.span()==at&&self.bounded_counter(symbol,&m.expression,m.span).is_some()=>{
                    !self.facts.targets.contains(&m.span)&&!self.facts.nonvalues.contains(&m.span)
                        &&!self.referenced_as_receiver(parent,m.span)
                }
                AstKind::StaticMemberExpression(m)if !m.optional&&m.object.span()==at&&m.property.name=="length"=>{
                    !self.facts.targets.contains(&m.span)&&!self.facts.nonvalues.contains(&m.span)
                        &&!self.referenced_as_receiver(parent,m.span)
                }
                AstKind::ComputedMemberExpression(m)if !m.optional&&m.object.span()==at&&matches!(unwrap(&m.expression),Expression::StringLiteral(k)if k.value=="length")=>{
                    !self.facts.targets.contains(&m.span)&&!self.facts.nonvalues.contains(&m.span)
                        &&!self.referenced_as_receiver(parent,m.span)
                }
                AstKind::StaticMemberExpression(m)if self.ordinary&&!m.optional&&m.object.span()==at&&m.property.name=="slice"=>matches!(self.nodes.parent_kind(parent),AstKind::CallExpression(c)if c.callee.span()==m.span&&!c.optional&&c.arguments.is_empty()),
                AstKind::ComputedMemberExpression(m)if self.ordinary&&!m.optional&&m.object.span()==at&&matches!(unwrap(&m.expression),Expression::StringLiteral(k)if k.value=="slice")=>matches!(self.nodes.parent_kind(parent),AstKind::CallExpression(c)if c.callee.span()==m.span&&!c.optional&&c.arguments.is_empty()),
                _=>false,
            }
        })
    }
    fn own_at(&self, symbol: SymbolId, index: usize, at: Span, depth: usize) -> bool {
        if depth == 0 || !self.locals.initialized(symbol, at) {
            return false;
        }
        let Some(origin) = self.arrays.get(&symbol) else {
            return false;
        };
        match *origin {
            Origin::Literal(len) => index < len,
            Origin::Slice(base, when) => self.own_at(base, index, when, depth - 1),
        }
    }
    fn length_at(&self, symbol: SymbolId, at: Span) -> Option<usize> {
        if !self.locals.initialized(symbol, at) {
            return None;
        }
        match *self.arrays.get(&symbol)? {
            Origin::Literal(len) => Some(len),
            // Own-cell bounds do not prove an exact length for arbitrary
            // species constructors. Literal origins need no such assumption.
            Origin::Slice(_, _) => None,
        }
    }
    fn body(&mut self, body: &[Statement<'_>]) {
        for statement in body {
            let Statement::VariableDeclaration(d) = statement else {
                continue;
            };
            if !matches!(
                d.kind,
                VariableDeclarationKind::Let | VariableDeclarationKind::Const
            ) || d.declarations.len() != 1
            {
                continue;
            }
            let (BindingPattern::BindingIdentifier(id), Some(init)) =
                (&d.declarations[0].id, &d.declarations[0].init)
            else {
                continue;
            };
            if !(id.name.starts_with("$SSA_") || id.name.starts_with("$CARRY_"))
                || self.protected.contains(id.name.as_str())
            {
                continue;
            }
            let Some(symbol) = id.symbol_id.get() else {
                continue;
            };
            let (array, index, counter, value) = if let Some((array, index)) =
                cell(init, self.scoping)
            {
                if !self.own_at(array, index, init.span(), 16) {
                    continue;
                }
                (
                    array,
                    Some(index),
                    None,
                    self.source[init.span().start as usize..init.span().end as usize].to_owned(),
                )
            } else if let Some((object, "length")) = member(init) {
                let Some(array) = analysis::identifier_symbol(object, self.scoping) else {
                    continue;
                };
                // Closed arrays have no length stores, deletes, receivers or
                // escapes. Every allowed indexed store remains in bounds,
                // so neither it nor arbitrary calls can change this length.
                let Some(len) = self.length_at(array, init.span()) else {
                    continue;
                };
                (array, None, None, len.to_string())
            } else if let Expression::ComputedMemberExpression(m) = unwrap(init) {
                if m.optional {
                    continue;
                }
                let Expression::Identifier(object) = unwrap(&m.object) else {
                    continue;
                };
                let Some(array) = analysis::identifier_symbol(object, self.scoping) else {
                    continue;
                };
                if !matches!(self.arrays.get(&array), Some(Origin::Literal(_))) {
                    continue;
                }
                let Some(counter) = self.bounded_counter(array, &m.expression, m.span) else {
                    continue;
                };
                (
                    array,
                    None,
                    Some(counter),
                    self.source[init.span().start as usize..init.span().end as usize].to_owned(),
                )
            } else {
                continue;
            };
            if !self.scoping.symbol_redeclarations(symbol).is_empty() {
                continue;
            }
            let refs: Vec<_> = self.scoping.get_resolved_references(symbol).collect();
            if refs.is_empty() {
                continue;
            }
            let name = self.scoping.symbol_ident(array);
            if refs.iter().any(|r| {
                let at = self.nodes.get_node(r.node_id()).kind().span();
                r.is_write()
                    || at.start < d.span.end
                    || !self.locals.initialized(symbol, at)
                    || (index.is_some() || counter.is_some())
                        && !self.locals.same_iteration(d.span, at)
                    || !self.locals.initialized(array, at)
                    || self.scoping.find_binding(r.scope_id(), name) != Some(array)
                    || counter.is_some_and(|s| {
                        self.scoping
                            .find_binding(r.scope_id(), self.scoping.symbol_ident(s))
                            != Some(s)
                    })
                    || self.facts.nonvalues.contains(&at)
                    || self.facts.stores.iter().any(|w| {
                        w.array == array
                            && (counter.is_some() || index == Some(w.index))
                            && w.span.end > d.span.end
                            && w.span.end <= at.start
                    })
            }) {
                continue;
            }
            // Rematerialization must shrink the output, rather than replacing
            // a short cache name with many long array-cell reads.
            if value.len() * refs.len() >= d.span.size() as usize + id.name.len() * refs.len() {
                continue;
            }
            let mut group = vec![analysis::Edit::replace(
                d.span,
                if self.facts.directive_barrier {
                    ";"
                } else {
                    ""
                },
            )];
            for r in refs {
                let at = self.nodes.get_node(r.node_id()).kind().span();
                let text = if self.facts.bare.contains(&at) {
                    format!("(0, {value})")
                } else {
                    format!("({value})")
                };
                group.push(analysis::Edit::replace(at, text));
            }
            if group.iter().any(|e| {
                self.edits
                    .iter()
                    .any(|p| e.start < p.end && p.start < e.end)
                    || self.source[e.start as usize..e.end as usize].contains("/*")
                    || self.source[e.start as usize..e.end as usize].contains("//")
            }) {
                continue;
            }
            self.edits.extend(group);
            self.count += 1;
        }
    }
}

#[cfg(test)]
mod bounded_tests {
    #[test]
    fn readonly_dense_key_lists_cover_ten_bounded_counter_forms() {
        for (test, update) in [
            ("i < $SSA_len", "i++"),
            ("i < $SSA_len", "++i"),
            ("i < $SSA_len", "i+=1"),
            ("i < $SSA_len", "i=i+1"),
            ("$SSA_len > i", "i++"),
            ("i <= $SSA_len-1", "i++"),
            ("$SSA_len-1 >= i", "i++"),
            ("i !== $SSA_len", "i++"),
            ("i != $SSA_len", "i++"),
            ("i < 2", "i++"),
        ] {
            let source=format!("function f(){{let $ARRAY_a=['a','b'];let $SSA_len=$ARRAY_a.length;let out=[];for(let i=0;{test};{update}){{let $SSA_key=$ARRAY_a[i];out.push($SSA_key,$SSA_key);}}return out;}}");
            let (code, count) =
                super::inline_private_array_reads(&source, &Default::default(), false).unwrap();
            assert_eq!(count, if test == "i < 2" { 1 } else { 2 }, "{code}");
            assert!(!code.contains("let $SSA_key"), "{code}");
        }
    }
    #[test]
    fn indexed_getter_escapes_counter_changes_and_receivers_are_not_private() {
        for middle in [
            "for(let i=0;i<=2;i++){use($ARRAY_a[i]);}",
            "for(let i=0;i<2;i++){i=2;use($ARRAY_a[i]);}",
            "for(let i=0;i<2;i++){change(()=>i++);use($ARRAY_a[i]);}",
            "for(let i=0;i<2;i++){use($ARRAY_a[i]());}",
            "for(let i=0;i<2;i++){use(($ARRAY_a[i])());}",
            "for(let i=0;i<2;i++){use($ARRAY_a[i]?.());}",
            "for(let i=0;i<2;i++){use($ARRAY_a[i]);$ARRAY_a.push(3);}",
            "for(let i=0;i<2;i++){delete $ARRAY_a[0];use($ARRAY_a[i]);}",
            "for(let i=0;i<2;i++){leak($ARRAY_a);use($ARRAY_a[i]);}",
            "use($ARRAY_a[unknown]);",
        ] {
            let source=format!("function f(){{let $ARRAY_a=['a','b'];let $SSA_len=$ARRAY_a.length;{middle}return [$SSA_len,$SSA_len];}}");
            let (code, count) =
                super::inline_private_array_reads(&source, &Default::default(), false).unwrap();
            assert_eq!(count, 0, "{code}");
        }
    }
}
