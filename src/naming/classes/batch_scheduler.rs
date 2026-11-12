//! Complete, ordered native BatchScheduler contract. p03 supplies only the
//! elapsed-clock factory witness; no weak scheduler role is promoted.
use crate::signals::extract::bindings::{property_key_name, Model};
use crate::signals::analysis::roles::FinderClaim;
#[cfg(test)]
use crate::signals::analysis::roles::FinderCtx;
use crate::signals::schema::Confidence;
use oxc_ast::{ast::*, AstKind};
use oxc_semantic::{Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Tree {
    Node(String, Vec<Tree>),
    Local(usize),
}
fn tree(tag: impl ToString, children: Vec<Tree>) -> Tree {
    Tree::Node(tag.to_string(), children)
}

/// Closed typed projection: only local binders are alpha-renamed. No nested
/// callable, unknown receiver, extra statement or unrecognized node is erased.
struct Body<'a> {
    scoping: &'a Scoping,
    locals: BTreeMap<SymbolId, usize>,
    budget: usize,
}
impl Body<'_> {
    fn bind(&mut self, pattern: &BindingPattern<'_>) -> Option<Tree> {
        let BindingPattern::BindingIdentifier(id) = pattern else {
            return None;
        };
        let symbol = id.symbol_id.get()?;
        if !self.scoping.symbol_redeclarations(symbol).is_empty()
            || self.locals.contains_key(&symbol)
        {
            return None;
        }
        let index = self.locals.len();
        self.locals.insert(symbol, index);
        Some(Tree::Local(index))
    }
    fn reference(&self, id: &IdentifierReference<'_>) -> Option<Tree> {
        match self
            .scoping
            .get_reference(id.reference_id.get()?)
            .symbol_id()
        {
            Some(s) => Some(Tree::Local(*self.locals.get(&s)?)),
            None if matches!(
                id.name.as_str(),
                "setTimeout" | "Number" | "Date" | "global" | "self" | "window"
            ) =>
            {
                Some(tree(format!("global:{}", id.name), vec![]))
            }
            _ => None,
        }
    }
    fn statements(&mut self, statements: &[Statement<'_>]) -> Option<Tree> {
        Some(tree(
            "body",
            statements
                .iter()
                .map(|s| self.statement(s))
                .collect::<Option<_>>()?,
        ))
    }
    fn function(&mut self, f: &Function<'_>) -> Option<Tree> {
        if f.r#async
            || f.generator
            || f.params.rest.is_some()
            || f.type_parameters.is_some()
            || f.return_type.is_some()
        {
            return None;
        }
        let mut nested = Body {
            scoping: self.scoping,
            locals: self.locals.clone(),
            budget: self.budget,
        };
        let mut params = vec![];
        for p in &f.params.items {
            if p.initializer.is_some() || !p.decorators.is_empty() {
                return None;
            }
            params.push(nested.bind(&p.pattern)?);
        }
        let body = f.body.as_ref()?;
        if !body.directives.is_empty() {
            return None;
        }
        let statements = nested.statements(&body.statements)?;
        self.budget = nested.budget;
        Some(tree("function", vec![tree("params", params), statements]))
    }
    fn statement(&mut self, s: &Statement<'_>) -> Option<Tree> {
        self.budget = self.budget.checked_sub(1)?;
        Some(match s {
            Statement::FunctionDeclaration(f) => {
                let id = f.id.as_ref()?;
                let symbol = id.symbol_id.get()?;
                if !self.scoping.symbol_redeclarations(symbol).is_empty()
                    || self.locals.contains_key(&symbol)
                {
                    return None;
                }
                let index = self.locals.len();
                self.locals.insert(symbol, index);
                tree(
                    "function-declaration",
                    vec![Tree::Local(index), self.function(f)?],
                )
            }
            Statement::BlockStatement(s) => self.statements(&s.body)?,
            Statement::ExpressionStatement(s) => {
                tree("expr", vec![self.expression(&s.expression)?])
            }
            Statement::ReturnStatement(s) => tree(
                "return",
                s.argument
                    .iter()
                    .map(|e| self.expression(e))
                    .collect::<Option<_>>()?,
            ),
            Statement::VariableDeclaration(s) => {
                let mut values = vec![];
                for d in &s.declarations {
                    values.push(self.bind(&d.id)?);
                    values.push(tree(
                        "init",
                        d.init
                            .iter()
                            .map(|e| self.expression(e))
                            .collect::<Option<_>>()?,
                    ));
                }
                tree(format!("decl:{:?}", s.kind), values)
            }
            Statement::IfStatement(s) => tree(
                "if",
                vec![
                    self.expression(&s.test)?,
                    self.statement(&s.consequent)?,
                    tree(
                        "else",
                        s.alternate
                            .iter()
                            .map(|s| self.statement(s))
                            .collect::<Option<_>>()?,
                    ),
                ],
            ),
            Statement::ForStatement(s) if s.init.is_none() => tree(
                "for",
                vec![
                    tree(
                        "test",
                        s.test
                            .iter()
                            .map(|e| self.expression(e))
                            .collect::<Option<_>>()?,
                    ),
                    tree(
                        "update",
                        s.update
                            .iter()
                            .map(|e| self.expression(e))
                            .collect::<Option<_>>()?,
                    ),
                    self.statement(&s.body)?,
                ],
            ),
            Statement::WhileStatement(s) => tree(
                "while",
                vec![self.expression(&s.test)?, self.statement(&s.body)?],
            ),
            Statement::BreakStatement(s) if s.label.is_none() => tree("break", vec![]),
            _ => return None,
        })
    }
    fn expression(&mut self, e: &Expression<'_>) -> Option<Tree> {
        self.budget = self.budget.checked_sub(1)?;
        Some(match e.without_parentheses() {
            Expression::Identifier(i) => self.reference(i)?,
            Expression::ThisExpression(_) => tree("this", vec![]),
            Expression::NumericLiteral(n) => tree(format!("number:{}", n.value), vec![]),
            Expression::StringLiteral(s) => tree(format!("string:{}", s.value), vec![]),
            Expression::NullLiteral(_) => tree("null", vec![]),
            Expression::BooleanLiteral(b) => tree(format!("bool:{}", b.value), vec![]),
            Expression::StaticMemberExpression(m) if !m.optional => tree(
                format!("member:{}", m.property.name),
                vec![self.expression(&m.object)?],
            ),
            Expression::CallExpression(c) if !c.optional && c.type_arguments.is_none() => {
                let mut values = vec![self.expression(&c.callee)?];
                for a in &c.arguments {
                    values.push(self.expression(a.as_expression()?)?);
                }
                tree("call", values)
            }
            Expression::NewExpression(n) if n.type_arguments.is_none() => {
                let mut values = vec![self.expression(&n.callee)?];
                for a in &n.arguments {
                    values.push(self.expression(a.as_expression()?)?);
                }
                tree("new", values)
            }
            Expression::BinaryExpression(b) => tree(
                format!("binary:{:?}", b.operator),
                vec![self.expression(&b.left)?, self.expression(&b.right)?],
            ),
            Expression::LogicalExpression(b) => tree(
                format!("logical:{:?}", b.operator),
                vec![self.expression(&b.left)?, self.expression(&b.right)?],
            ),
            Expression::UnaryExpression(u) => tree(
                format!("unary:{:?}", u.operator),
                vec![self.expression(&u.argument)?],
            ),
            Expression::ConditionalExpression(c) => tree(
                "conditional",
                vec![
                    self.expression(&c.test)?,
                    self.expression(&c.consequent)?,
                    self.expression(&c.alternate)?,
                ],
            ),
            Expression::AssignmentExpression(a) => tree(
                format!("assign:{:?}", a.operator),
                vec![self.target(&a.left)?, self.expression(&a.right)?],
            ),
            Expression::UpdateExpression(u) => {
                let SimpleAssignmentTarget::AssignmentTargetIdentifier(id) = &u.argument else {
                    return None;
                };
                tree(
                    format!("update:{:?}:{}", u.operator, u.prefix),
                    vec![self.reference(id)?],
                )
            }
            _ => return None,
        })
    }
    fn target(&mut self, t: &AssignmentTarget<'_>) -> Option<Tree> {
        match t {
            AssignmentTarget::AssignmentTargetIdentifier(i) => self.reference(i),
            AssignmentTarget::StaticMemberExpression(m) if !m.optional => Some(tree(
                format!("member:{}", m.property.name),
                vec![self.expression(&m.object)?],
            )),
            _ => None,
        }
    }
}
fn method(scoping: &Scoping, f: &Function<'_>) -> Option<Tree> {
    if f.r#async
        || f.generator
        || f.params.rest.is_some()
        || f.type_parameters.is_some()
        || f.return_type.is_some()
    {
        return None;
    }
    let mut proof = Body {
        scoping,
        locals: BTreeMap::new(),
        budget: 512,
    };
    let mut params = vec![];
    for p in &f.params.items {
        if p.initializer.is_some() || !p.decorators.is_empty() {
            return None;
        }
        params.push(proof.bind(&p.pattern)?);
    }
    let body = f.body.as_ref()?;
    if !body.directives.is_empty() {
        return None;
    }
    Some(tree(
        "method",
        vec![tree("params", params), proof.statements(&body.statements)?],
    ))
}

const HOST: &str = "typeof global !== 'undefined' && global != null && global.global === global ? global : typeof self !== 'undefined' && self != null && self.self === self ? self : typeof window !== 'undefined' && window != null && window.window === window ? window : void 0";
const NOW: &str = "now() { return this._internalNow(); }";
const SCHEDULE: &str = "schedule(work) { setTimeout(this._process.bind(this, work), 0); }";
const PROCESS: &str = r#"_process(work) {
    let budget = this.targetBatchTimeMs;
    let start = this.now();
    let mean = 0;
    let count = 0;
    let pending;
    for (;;) {
        let before = this.now();
        pending = work();
        let after = this.now();
        mean = (mean * count + (after - before)) / (count + 1);
        ++count;
        if (!(after + mean < start + budget ? pending : after + mean < start + budget)) break;
    }
    if (pending) { setTimeout(this._process.bind(this, work), 0); }
}"#;

// These are distinct protocols, not algebraic rewrites of PROCESS. In
// particular the delayed store observes the incremented count at the deadline
// and commits the mean only on the continuing edge.
const CARRIED_PROCESS: &str = r#"_process(work) {
    let budget = this.targetBatchTimeMs;
    let start = this.now();
    let count = 0;
    let mean = 0;
    let pending, next;
    for (;; mean = next) {
        let before = this.now();
        pending = work();
        let after = this.now();
        next = (mean * count + (after - before)) / (count + 1);
        ++count;
        if (!(after + next < start + budget ? pending : after + next < start + budget)) break;
    }
    if (pending) { setTimeout(this._process.bind(this, work), 0); }
}"#;
const DELAYED_PROCESS: &str = r#"_process(work) {
    let budget = this.targetBatchTimeMs;
    let start = this.now();
    let mean = 0;
    let count = 0;
    let pending;
    for (;;) {
        let before = this.now();
        pending = work();
        let after = this.now();
        ++count;
        if (!(after + (mean * count + (after - before)) / (count + 1) < start + budget ? pending : after + (mean * count + (after - before)) / (count + 1) < start + budget)) break;
        mean = (mean * count + (after - before)) / (count + 1);
    }
    if (pending) { setTimeout(this._process.bind(this, work), 0); }
}"#;
// The deadline and continuing-edge mean both observe the old count. The
// lowered increment is committed only by the for-update after that mean store.
// Keep this protocol distinct from DELAYED_PROCESS's pre-test increment.
const EDGE_COUNT_PROCESS: &str = r#"_process(work) {
    let budget = this.targetBatchTimeMs;
    let start = this.now();
    let mean = 0;
    let count = 0;
    let pending, next;
    for (;; count = next) {
        let before = this.now();
        pending = work();
        let after = this.now();
        next = Number(count) + 1;
        if (!(after + (mean * count + (after - before)) / (count + 1) < start + budget ? pending : after + (mean * count + (after - before)) / (count + 1) < start + budget)) break;
        mean = (mean * count + (after - before)) / (count + 1);
    }
    if (pending) { setTimeout(this._process.bind(this, work), 0); }
}"#;
// The coercion is an observable call after the mean store and before the
// deadline. Only the continuing for-update edge adds one to that saved value.
// Do not normalize this into Number(count) + 1 or move it across clock/work.
const UPDATE_ADD_PROCESS: &str = r#"_process(work) {
    let budget = this.targetBatchTimeMs;
    let start = this.now();
    let mean = 0;
    let count = 0;
    let pending, next;
    for (;; count = next + 1) {
        let before = this.now();
        pending = work();
        let after = this.now();
        mean = (mean * count + (after - before)) / (count + 1);
        next = Number(count);
        if (!(after + mean < start + budget ? pending : after + mean < start + budget)) break;
    }
    if (pending) { setTimeout(this._process.bind(this, work), 0); }
}"#;
const HOST_BRANCH: &str = r#"host() {
    let host;
    if (typeof global !== 'undefined' && global != null && global.global === global) {
        host = global;
    } else if (typeof self !== 'undefined' && self != null && self.self === self) {
        host = self;
    } else if (typeof window !== 'undefined' && window != null && window.window === window) {
        host = window;
    } else { host = void 0; }
    return host;
}"#;

const STAGED_GLOBAL_HOST: &str = r#"host() {
    let safe;
    if (typeof global !== 'undefined') { safe = global != null; }
    else { safe = false; }
    return safe && global.global === global ? global : typeof self !== 'undefined' && self != null && self.self === self ? self : typeof window !== 'undefined' && window != null && window.window === window ? window : void 0;
}"#;

// Independently close the whole clock factory for newly admitted constructors.
// The older finder claim is still required, but its member inventory alone is
// insufficient to prove native purpose, baseline capture, or immutable flow.
const NATIVE_CLOCK: &str = r#"clock(host) {
    let native;
    if (host && host.performance && typeof host.performance.now === 'function') {
        native = host.performance.now.bind(host.performance);
    } else {
        function fallback() { return Number(new Date()); }
        native = fallback;
    }
    let captured = native;
    let baseline = native();
    function elapsed() { return captured() - baseline; }
    elapsed.start = baseline;
    return elapsed;
}"#;

fn native_clock(model: &Model<'_, '_>, span: Span) -> bool {
    model
        .nodes
        .iter()
        .find_map(|n| match n.kind() {
            AstKind::Function(f) if f.span == span => method(model.scoping, f),
            _ => None,
        })
        .is_some_and(|actual| actual == grammar().native_clock)
}

fn reference(source: &str) -> Tree {
    let allocator = oxc_allocator::Allocator::default();
    let parsed =
        oxc_parser::Parser::new(&allocator, source, oxc_span::SourceType::default()).parse();
    assert!(parsed.errors.is_empty(), "invalid scheduler reference");
    let semantic = oxc_semantic::SemanticBuilder::new()
        .build(&parsed.program)
        .semantic;
    let Statement::ClassDeclaration(c) = &parsed.program.body[0] else {
        unreachable!()
    };
    let ClassElement::MethodDefinition(m) = &c.body.body[0] else {
        unreachable!()
    };
    method(semantic.scoping(), &m.value).expect("closed scheduler reference")
}
struct Grammar {
    now: Tree,
    schedule: Tree,
    process: Vec<(Tree, Option<&'static str>)>,
    host: Tree,
    host_branch: Tree,
    staged_global_host: Tree,
    native_clock: Tree,
}
fn grammar() -> &'static Grammar {
    static GRAMMAR: OnceLock<Grammar> = OnceLock::new();
    GRAMMAR.get_or_init(|| {
        let mut process = vec![];
        // Explicit semantic variants: conjunction lowering, while/for, direct
        // or lowered increment, and a separately stored elapsed duration.
        for condition in ["after + mean < start + budget ? pending : after + mean < start + budget", "after + mean < start + budget && pending"] {
            for loop_head in ["for (;;)", "while (true)"] {
                for increment in [0, 1, 2] {
                    for duration_temp in [false, true] {
                        let mut source = PROCESS.replace("after + mean < start + budget ? pending : after + mean < start + budget", condition).replace("for (;;)", loop_head);
                        if increment > 0 {
                            source = source.replace("let pending;", "let pending, next;").replace("++count;", "next = Number(count) + 1;");
                            if increment == 1 && loop_head == "for (;;)" { source = source.replace("for (;;)", "for (;; count = next)"); }
                            else { source = source.replace("if (!(", "count = next; if (!("); /* excluded: changed deadline count is unobserved */ }
                        }
                        if duration_temp { source = source.replace("mean = (mean * count + (after - before))", "let elapsed = after - before; mean = (mean * count + elapsed)"); }
                        process.push((reference(&format!("class Pattern {{ {source} }}")), None));
                    }
                }
            }
        }
        // A separate closed production for the observed direct-store protocol.
        // Only these two literal-zero declarations differ in order; retain the
        // role-specific arithmetic and every effectful initializer/call in place.
        let count_first = PROCESS.replace(
            "let mean = 0;\n    let count = 0;",
            "let count = 0;\n    let mean = 0;",
        );
        process.push((reference(&format!("class Pattern {{ {count_first} }}")), None));
        process.push((reference(&format!("class Pattern {{ {CARRIED_PROCESS} }}")), Some("next mean used by deadline then carried on for-update edge")));
        process.push((reference(&format!("class Pattern {{ {DELAYED_PROCESS} }}")), Some("increment before inline deadline mean; delayed mean store on continuing edge only")));
        process.push((reference(&format!("class Pattern {{ {EDGE_COUNT_PROCESS} }}")), Some("old count at inline deadline and continuing-edge mean store; lowered count committed on for-update edge")));
        process.push((reference(&format!("class Pattern {{ {UPDATE_ADD_PROCESS} }}")), Some("mean stored before counter coercion and deadline; saved numeric count plus one committed only on continuing for-update edge")));
        Grammar { now: reference(&format!("class Pattern {{ {NOW} }}")), schedule: reference(&format!("class Pattern {{ {SCHEDULE} }}")), process, host: reference(&format!("class Pattern {{ host() {{ return {HOST}; }} }}")), host_branch: reference(&format!("class Pattern {{ {HOST_BRANCH} }}")), staged_global_host: reference(&format!("class Pattern {{ {STAGED_GLOBAL_HOST} }}")), native_clock: reference(&format!("class Pattern {{ {NATIVE_CLOCK} }}")) }
    })
}

/// Resolve only a stable, ordered callable origin. Unlike a latest-value
/// resolver this rejects mutation, redeclarations and conditional assignments.
fn clock_origin(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    before: u32,
    seen: &mut BTreeSet<SymbolId>,
) -> Option<Span> {
    let Expression::Identifier(id) = e.without_parentheses() else {
        return None;
    };
    let symbol = crate::analysis::identifier_symbol(id, model.scoping)?;
    if !seen.insert(symbol)
        || seen.len() > 16
        || !model.scoping.symbol_redeclarations(symbol).is_empty()
    {
        return None;
    }
    match model.nodes.kind(model.scoping.symbol_declaration(symbol)) {
        AstKind::Function(f) if model.binding_writes(symbol) == 0 => Some(f.span),
        AstKind::VariableDeclarator(d) => {
            if let Some(init) = &d.init {
                if model.binding_writes(symbol) != 0 || d.span.end > before {
                    return None;
                }
                clock_origin(model, init, d.span.start, seen)
            } else {
                if model.binding_writes(symbol) != 1 {
                    return None;
                }
                // Only an unconditional assignment directly in the declaring
                // function's body can initialize a lifted clock alias.
                let value = model.symbol_assigned_value(symbol)?;
                let assignment = model.nodes.iter().find(|n| matches!(n.kind(), AstKind::AssignmentExpression(a) if a.right.span() == value.span()))?;
                let AstKind::AssignmentExpression(a) = assignment.kind() else {
                    return None;
                };
                if a.operator != oxc_syntax::operator::AssignmentOperator::Assign
                    || a.span.end > before
                {
                    return None;
                }
                let statement = model.nodes.parent_node(assignment.id());
                if !matches!(statement.kind(), AstKind::ExpressionStatement(_))
                    || !matches!(
                        model.nodes.parent_kind(statement.id()),
                        AstKind::FunctionBody(_)
                    )
                    || model.enclosing_function(assignment.id()).map(|f| f.span)
                        != model
                            .enclosing_function(model.scoping.symbol_declaration(symbol))
                            .map(|f| f.span)
                {
                    return None;
                }
                clock_origin(model, value, a.span.start, seen)
            }
        }
        _ => None,
    }
}

fn constructor(
    model: &Model<'_, '_>,
    f: &Function<'_>,
    elapsed: &BTreeSet<(u32, u32)>,
) -> Option<()> {
    if f.r#async || f.generator || f.params.rest.is_some() || f.params.items.len() != 1 {
        return None;
    }
    let p = &f.params.items[0];
    if p.initializer.is_some() {
        return None;
    }
    let BindingPattern::BindingIdentifier(p) = &p.pattern else {
        return None;
    };
    let parameter = p.symbol_id.get()?;
    if model.binding_writes(parameter) != 0 {
        return None;
    }
    let b = f.body.as_ref()?;
    if !b.directives.is_empty() || !matches!(b.statements.len(), 2 | 4) {
        return None;
    }
    let assignment = |index| {
        let Statement::ExpressionStatement(s) = &b.statements[index] else {
            return None;
        };
        let Expression::AssignmentExpression(a) = s.expression.without_parentheses() else {
            return None;
        };
        (a.operator == oxc_syntax::operator::AssignmentOperator::Assign).then_some(a)
    };
    let field = |a: &AssignmentExpression<'_>, key: &str| matches!(&a.left, AssignmentTarget::StaticMemberExpression(m) if !m.optional && m.property.name == key && matches!(m.object.without_parentheses(), Expression::ThisExpression(_)));
    let budget = assignment(0)?;
    if !field(budget, "targetBatchTimeMs")
        || crate::analysis::expression_symbol(budget.right.without_parentheses(), model.scoping)
            != Some(parameter)
    {
        return None;
    }
    let clock = assignment(b.statements.len() - 1)?;
    if !field(clock, "_internalNow") {
        return None;
    }
    let Expression::CallExpression(c) = clock.right.without_parentheses() else {
        return None;
    };
    if c.optional || c.arguments.len() != 1 || c.type_arguments.is_some() {
        return None;
    }
    let span = clock_origin(model, &c.callee, f.span.start, &mut BTreeSet::new())?;
    if !elapsed.contains(&(span.start, span.end)) {
        return None;
    }
    let mut projection = Body {
        scoping: model.scoping,
        locals: BTreeMap::new(),
        budget: 256,
    };
    let mut host_statements = vec![];
    if b.statements.len() == 4 {
        // Project every node of the declaration and exhaustive selection. No
        // extra effect, alternate fallback, shadowed global or escaped local
        // can be dropped by this closed projection.
        for statement in &b.statements[1..3] {
            host_statements.push(projection.statement(statement)?);
        }
    }
    let actual = projection.expression(c.arguments[0].as_expression()?)?;
    // Extract the one return expression from the separately parsed host method.
    host_statements.push(tree("return", vec![actual]));
    let expected = tree(
        "method",
        vec![tree("params", vec![]), tree("body", host_statements)],
    );
    let g = grammar();
    let host = if b.statements.len() == 2 {
        &g.host
    } else {
        &g.host_branch
    };
    if &expected == host {
        return Some(());
    }
    // No syntax erasure: match every staged assignment and branch, then prove
    // the same scoped callable's complete native implementation independently.
    (b.statements.len() == 4 && expected == g.staged_global_host && native_clock(model, span))
        .then_some(())
}

// Constructor values may flow through stable direct aliases and into `new`.
// Member access/escaping calls could mutate the proved API, so refuse them.
fn class_uses(
    model: &Model<'_, '_>,
    symbol: SymbolId,
    ready: Span,
    assigned: bool,
    seen: &mut BTreeSet<SymbolId>,
) -> bool {
    if !seen.insert(symbol)
        || seen.len() > 32
        || model.binding_writes(symbol) != usize::from(assigned)
        || !model.scoping.symbol_redeclarations(symbol).is_empty()
    {
        return false;
    }
    model.scoping.get_resolved_references(symbol).all(|r| {
        let AstKind::IdentifierReference(id) = model.nodes.kind(r.node_id()) else {
            return false;
        };
        if r.is_write() {
            return assigned && ready.contains_inclusive(id.span);
        }
        match model.nodes.parent_kind(r.node_id()) {
            AstKind::NewExpression(n) => {
                matches!(&n.callee, Expression::Identifier(i) if i.span == id.span)
            }
            AstKind::VariableDeclarator(d)
                if matches!(&d.init, Some(Expression::Identifier(i)) if i.span == id.span)
                    && d.span.start > ready.end =>
            {
                let BindingPattern::BindingIdentifier(alias) = &d.id else {
                    return false;
                };
                alias
                    .symbol_id
                    .get()
                    .is_some_and(|s| class_uses(model, s, d.span, false, seen))
            }
            AstKind::AssignmentExpression(a)
                if a.operator == oxc_syntax::operator::AssignmentOperator::Assign
                    && a.right.span() == id.span
                    && a.span.start > ready.end =>
            {
                let AssignmentTarget::AssignmentTargetIdentifier(alias) = &a.left else {
                    return false;
                };
                let Some(s) = crate::analysis::identifier_symbol(alias, model.scoping) else {
                    return false;
                };
                let AstKind::VariableDeclarator(d) =
                    model.nodes.kind(model.scoping.symbol_declaration(s))
                else {
                    return false;
                };
                if d.init.is_some() {
                    return false;
                }
                let statement = model.nodes.parent_node(model.nodes.parent_id(r.node_id()));
                matches!(statement.kind(), AstKind::ExpressionStatement(_))
                    && matches!(
                        model.nodes.parent_kind(statement.id()),
                        AstKind::FunctionBody(_)
                    )
                    && model.enclosing_function(statement.id()).map(|f| f.span)
                        == model
                            .enclosing_function(model.scoping.symbol_declaration(s))
                            .map(|f| f.span)
                    && class_uses(model, s, a.span, true, seen)
            }
            _ => false,
        }
    })
}

#[cfg(test)]
pub(crate) fn claims(model: &Model<'_, '_>) -> Vec<FinderClaim> {
    let Some(program) = model.nodes.iter().find_map(|n| {
        if let AstKind::Program(p) = n.kind() {
            Some(p)
        } else {
            None
        }
    }) else {
        return vec![];
    };
    let ctx = FinderCtx {
        source: model.source,
        program,
        scoping: model.scoping,
        nodes: model.nodes,
    };
    claims_with_finder_claims(model, &crate::signals::analysis::roles::vm_exec::classes::find(&ctx))
}

/// Integration supplies the finder claims collected for this exact Model.
/// Recognition is identical to `claims`, without collecting p03 a second time.
pub(crate) fn claims_with_finder_claims(
    model: &Model<'_, '_>,
    finder_claims: &[FinderClaim],
) -> Vec<FinderClaim> {
    let elapsed = finder_claims
        .iter()
        .filter_map(|c| match c {
            FinderClaim::Function {
                role: "elapsed_timer_factory",
                function_span,
                ..
            } => Some((function_span.start, function_span.end)),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let mut result = vec![];
    for node in model.nodes.iter() {
        let AstKind::Class(c) = node.kind() else {
            continue;
        };
        if c.r#type != ClassType::ClassDeclaration
            || c.super_class.is_some()
            || !c.decorators.is_empty()
            || c.body.body.len() != 4
        {
            continue;
        }
        let Some(symbol) = c.id.as_ref().and_then(|i| i.symbol_id.get()) else {
            continue;
        };
        if model.binding_writes(symbol) != 0
            || !model.scoping.symbol_redeclarations(symbol).is_empty()
        {
            continue;
        }
        let mut own = BTreeMap::new();
        let valid = c.body.body.iter().all(|e| {
            let ClassElement::MethodDefinition(m) = e else {
                return false;
            };
            let Some(key) = property_key_name(&m.key) else {
                return false;
            };
            let kind = if key == "constructor" {
                MethodDefinitionKind::Constructor
            } else {
                MethodDefinitionKind::Method
            };
            !m.computed
                && !m.r#static
                && m.decorators.is_empty()
                && m.kind == kind
                && own.insert(key, m.value.as_ref()).is_none()
        });
        if !valid {
            continue;
        }
        let accepted = (|| {
            constructor(model, own.get("constructor")?, &elapsed)?;
            let g = grammar();
            if method(model.scoping, own.get("now")?)? != g.now
                || method(model.scoping, own.get("schedule")?)? != g.schedule
            {
                return None;
            }
            let process = method(model.scoping, own.get("_process")?)?;
            g.process
                .iter()
                .find_map(|(pattern, evidence)| (pattern == &process).then_some(*evidence))
        })();
        if let Some(protocol) =
            accepted.filter(|_| class_uses(model, symbol, c.span, false, &mut BTreeSet::new()))
        {
            // Old direct-store claims are persisted as class explanations.
            // Preserve their exact evidence bytes, including punctuation.
            let mut evidence = "unique own constructor/now/schedule/_process; stable scoped elapsed-clock origin and host selection; budget formal; zero-delay receiver/work binding; ordered clock-work-clock, positive elapsed running mean, deadline/work short circuit and pending-only reschedule".to_string();
            if let Some(protocol) = protocol {
                evidence.push_str("; ");
                evidence.push_str(protocol);
            }
            result.push(FinderClaim::ClassBinding {
                role: "owned_batch_scheduler_class",
                class_span: c.span,
                confidence: Confidence::High,
                evidence: vec![evidence],
            });
        }
    }
    result
}

#[cfg(test)]
#[path = "batch_scheduler_tests.rs"]
mod tests;
