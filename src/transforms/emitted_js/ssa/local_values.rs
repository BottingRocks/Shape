//! Local value numbering and producer/consumer fusion. Expressions are compared
//! by resolved bindings and reaching store versions, never by generated names.
//! Only expressions proven non-throwing and free of coercion hooks can be reused.
use std::collections::{BTreeMap, BTreeSet};

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::scope::ScopeFlags;

use super::super::{adjacent_ssa::LocalReads, index::SpanIndex};
use crate::analysis;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Number,
    // Number, Boolean, null or undefined; arithmetic cannot invoke coercion
    // hooks and addition with another value in this category stays numeric.
    NumericPrimitive,
    String,
    Boolean,
    Null,
    Undefined,
    BigInt,
    Unknown,
}
impl Kind {
    fn coercible(self) -> bool {
        !matches!(self, Self::Unknown | Self::BigInt)
    }
}
#[derive(Clone, PartialEq, Eq)]
struct Value {
    version: u32,
    kind: Kind,
}
type Values = BTreeMap<SymbolId, Value>;
#[derive(Clone)]
struct Pure {
    key: String,
    kind: Kind,
    reads: Vec<(SymbolId, Span)>,
    versions: Vec<(SymbolId, u32)>,
}
impl Pure {
    fn compound(&self) -> bool {
        !self.reads.is_empty() && self.key.starts_with('(')
    }
}
#[derive(Clone)]
struct Context {
    span: Span,
    read: Span,
    key: String,
}
#[derive(Clone)]
struct Definition {
    init: Span,
    name: String,
}

pub(crate) fn simplify_local_values(
    source: &str,
    protected: &BTreeSet<String>,
) -> Result<(String, usize), String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut locals = LocalReads::default();
    locals.visit_program(&program);
    locals.build(&scoping, &nodes, protected);
    let mut regions = Regions::default();
    regions.visit_program(&program);
    if regions.dynamic {
        return Ok((source.into(), 0));
    }
    regions.functions.build();
    let mut scan = Scan {
        source,
        scoping: &scoping,
        nodes: &nodes,
        locals: &locals,
        regions: &regions,
        protected,
        edits: Vec::new(),
        definitions: BTreeMap::new(),
        contexts: BTreeMap::new(),
        immutable: Values::new(),
        pure_definitions: BTreeMap::new(),
        stable_uses: BTreeSet::new(),
    };
    for _ in 0..16 {
        let before = scan.immutable.clone();
        Discover(&mut scan).visit_program(&program);
        if before == scan.immutable {
            break;
        }
    }
    scan.visit_program(&program);
    let substitutions = scan.substitutions();
    // Fuse any identical pure consumer of an immutable local, provided it is
    // the only way that value is observed. One reference per consumer ensures
    // inserting the producer does not duplicate its calls or allocations.
    let fused = scan.fusions();
    // Fusion takes precedence; apply other rewrites on the next fixed-point
    // round so nested producer/consumer edits never overlap.
    let mut edits = if !fused.is_empty() {
        fused
    } else if !substitutions.is_empty() {
        substitutions
    } else {
        scan.edits
    };
    edits.sort_by_key(|e| (e.start, std::cmp::Reverse(e.end)));
    let mut end = 0;
    edits.retain(|e| {
        if e.start < end {
            false
        } else {
            end = e.end;
            true
        }
    });
    let count = edits.len();
    let applied = analysis::apply_edits(source, edits);
    if !applied.rejected.is_empty() {
        return Err("local value edits overlap".into());
    }
    Ok((applied.output, count))
}

impl Scan<'_, '_> {
    /// Substitute each pure definition at its reads where every read is
    /// dominated and owned and its inputs keep their values there.
    fn substitutions(&self) -> Vec<analysis::Edit> {
        let mut substitutions = Vec::<analysis::Edit>::new();
        for (symbol, def) in &self.definitions {
            let Some(pure) = self.pure_definitions.get(symbol) else {
                continue;
            };
            let refs: Vec<_> = self.scoping.get_resolved_references(*symbol).collect();
            if refs.is_empty() || refs.iter().any(|r| r.is_write()) {
                continue;
            }
            // Keep computations over stored inputs cached. Literal-only values
            // have no snapshot dependencies; their small canonical forms (signed
            // numbers, negative zero, infinities) may be propagated freely.
            if refs.len() > 1 && pure.key.starts_with('(') && !pure.reads.is_empty() {
                continue;
            }
            let value = &self.source[def.init.start as usize..def.init.end as usize];
            let Some(decl) = self.regions.declarations.get(symbol) else {
                continue;
            };
            // Keep expansion bounded and require an actual reduction in output.
            if value.len() > 160
                || value.len() * refs.len()
                    >= decl.0.size() as usize + self.scoping.symbol_name(*symbol).len() * refs.len()
            {
                continue;
            }
            let mut group = vec![analysis::Edit::replace(
                decl.0,
                if self.regions.directive_barrier { ";" } else { "" },
            )];
            let mut valid = true;
            for reference in refs {
                let span = self.nodes.get_node(reference.node_id()).kind().span();
                if !self.dominated(*symbol, span)
                    || !self.owned(*symbol, span)
                    || self.regions.non_values.contains(&span)
                    || (!self.stable_uses.contains(&span)
                        && pure
                            .reads
                            .iter()
                            .any(|(s, _)| !self.immutable.contains_key(s)))
                    || pure.reads.iter().any(|(dependency, _)| {
                        !self.owned(*dependency, span)
                            || (self.immutable.contains_key(dependency)
                                && !self.dominated(*dependency, span))
                            || self.scoping
                                .find_binding(reference.scope_id(), self.scoping.symbol_ident(*dependency))
                                != Some(*dependency)
                    })
                {
                    valid = false;
                    break;
                }
                group.push(analysis::Edit::replace(span, format!("({value})")));
            }
            if valid
                && !group.iter().any(|e| {
                    substitutions
                        .iter()
                        .any(|p| e.start < p.end && p.start < e.end)
                })
            {
                substitutions.extend(group);
            }
        }
        substitutions
    }

    /// Fuse the one identical pure consumer of each immutable local into its
    /// definition.
    fn fusions(&self) -> Vec<analysis::Edit> {
        let mut fused = Vec::new();
        for (symbol, contexts) in &self.contexts {
            let Some(def) = self.definitions.get(symbol) else {
                continue;
            };
            let refs: Vec<_> = self.scoping.get_resolved_references(*symbol).collect();
            if refs.len() != contexts.len()
                || refs.iter().any(|r| r.is_write())
                || !self.scoping.symbol_redeclarations(*symbol).is_empty()
                || contexts.is_empty()
                || contexts
                    .values()
                    .any(|c| c.key != contexts.values().next().unwrap().key)
                || contexts.values().any(|c| c.span.start <= def.init.end)
            {
                continue;
            }
            let context = contexts.values().next().unwrap();
            let mut consumer =
                self.source[context.span.start as usize..context.span.end as usize].to_string();
            consumer.replace_range(
                (context.read.start - context.span.start) as usize
                    ..(context.read.end - context.span.start) as usize,
                &format!(
                    "({})",
                    &self.source[def.init.start as usize..def.init.end as usize]
                ),
            );
            let mut group = vec![analysis::Edit::replace(def.init, format!("({consumer})"))];
            for context in contexts.values() {
                group.push(analysis::Edit::replace(context.span, &def.name));
            }
            if group.iter().any(|e| {
                fused
                    .iter()
                    .any(|other: &analysis::Edit| e.start < other.end && other.start < e.end)
            }) {
                continue;
            }
            fused.extend(group);
        }
        fused
    }
}

#[derive(Default)]
struct Regions {
    function_names: BTreeSet<String>,
    functions: SpanIndex,
    dynamic: bool,
    declarations: BTreeMap<SymbolId, (Span, Span)>,
    non_values: BTreeSet<Span>,
    directive_barrier: bool,
}
impl Regions {
    fn declarations(&mut self, statements: &[Statement<'_>], body: Span) {
        for s in statements {
            if let Statement::VariableDeclaration(d) = s {
                if d.declarations.len() != 1 {
                    continue;
                }
                if let BindingPattern::BindingIdentifier(id) = &d.declarations[0].id {
                    if let Some(symbol) = id.symbol_id.get() {
                        self.declarations
                            .insert(symbol, (d.span, Span::new(d.span.end, body.end)));
                    }
                }
            }
        }
    }
}
impl<'a> Visit<'a> for Regions {
    fn visit_expression_statement(&mut self, s: &ExpressionStatement<'a>) {
        self.directive_barrier |= matches!(s.expression, Expression::StringLiteral(_));
        walk::walk_expression_statement(self, s);
    }
    fn visit_function_body(&mut self, b: &FunctionBody<'a>) {
        self.declarations(&b.statements, b.span);
        walk::walk_function_body(self, b);
    }
    fn visit_block_statement(&mut self, b: &BlockStatement<'a>) {
        self.declarations(&b.body, b.span);
        walk::walk_block_statement(self, b);
    }
    fn visit_object_property(&mut self, p: &ObjectProperty<'a>) {
        if p.shorthand {
            self.non_values.insert(p.value.span());
        }
        walk::walk_object_property(self, p);
    }
    fn visit_unary_expression(&mut self, u: &UnaryExpression<'a>) {
        if u.operator == UnaryOperator::Delete {
            let mut argument = &u.argument;
            while let Expression::ParenthesizedExpression(p) = argument {
                argument = &p.expression;
            }
            self.non_values.insert(argument.span());
        }
        walk::walk_unary_expression(self, u);
    }
    fn visit_function(&mut self, f: &Function<'a>, flags: ScopeFlags) {
        if let Some(id) = &f.id {
            self.function_names.insert(id.name.to_string());
        }
        self.functions.push(f.span);
        walk::walk_function(self, f, flags);
    }
    fn visit_arrow_function_expression(&mut self, f: &ArrowFunctionExpression<'a>) {
        self.functions.push(f.span);
        walk::walk_arrow_function_expression(self, f);
    }
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        self.dynamic |= id.name == "eval";
    }
    fn visit_with_statement(&mut self, s: &WithStatement<'a>) {
        self.dynamic = true;
        walk::walk_with_statement(self, s);
    }
}
struct Scan<'s, 'a> {
    source: &'s str,
    scoping: &'s Scoping,
    nodes: &'s AstNodes<'a>,
    locals: &'s LocalReads,
    regions: &'s Regions,
    protected: &'s BTreeSet<String>,
    edits: Vec<analysis::Edit>,
    definitions: BTreeMap<SymbolId, Definition>,
    contexts: BTreeMap<SymbolId, BTreeMap<u32, Context>>,
    immutable: Values,
    pure_definitions: BTreeMap<SymbolId, Pure>,
    stable_uses: BTreeSet<Span>,
}
impl<'a> Scan<'_, 'a> {
    fn dominated(&self, symbol: SymbolId, at: Span) -> bool {
        self.regions
            .declarations
            .get(&symbol)
            .is_some_and(|(_, region)| region.start <= at.start && at.end <= region.end)
    }
    fn owned(&self, symbol: SymbolId, at: Span) -> bool {
        // Sloppy block functions can implicitly assign an outer var.
        // Those writes are not identifier references in the AST.
        !(self.protected.contains(self.scoping.symbol_name(symbol))
            || self
                .scoping
                .symbol_flags(symbol)
                .is_function_scoped_declaration()
                && self
                    .regions
                    .function_names
                    .contains(self.scoping.symbol_name(symbol)))
            && self.locals.initialized(symbol, at)
            && self
                .regions
                .functions
                .enclosing(self.scoping.symbol_span(symbol))
                == self.regions.functions.enclosing(at)
    }
    fn pure(&self, e: &Expression<'_>, values: &Values) -> Option<Pure> {
        let literal = |key, kind| {
            Some(Pure {
                key,
                kind,
                reads: Vec::new(),
                versions: Vec::new(),
            })
        };
        match e {
            Expression::NumericLiteral(n) => {
                literal(format!("n{:x}", n.value.to_bits()), Kind::Number)
            }
            Expression::StringLiteral(s) => literal(format!("s{:?}", s.value), Kind::String),
            Expression::BooleanLiteral(b) => literal(format!("b{}", b.value), Kind::Boolean),
            Expression::NullLiteral(_) => literal("null".into(), Kind::Null),
            Expression::BigIntLiteral(b) => literal(
                format!(
                    "big{}",
                    &self.source[b.span.start as usize..b.span.end as usize]
                ),
                Kind::BigInt,
            ),
            Expression::Identifier(id) => {
                let symbol = analysis::identifier_symbol(id, self.scoping)?;
                let value = values.get(&symbol).or_else(|| {
                    self.immutable
                        .get(&symbol)
                        .filter(|_| self.dominated(symbol, id.span))
                })?;
                if !self.owned(symbol, id.span) {
                    return None;
                }
                Some(Pure {
                    key: format!("v{}:{}", symbol.index(), value.version),
                    kind: if value.kind == Kind::Unknown
                        && self.locals.numeric_cell(symbol, id.span)
                    {
                        Kind::NumericPrimitive
                    } else {
                        value.kind
                    },
                    reads: vec![(symbol, id.span)],
                    versions: vec![(symbol, value.version)],
                })
            }
            Expression::ParenthesizedExpression(p) => self.pure(&p.expression, values),
            Expression::UnaryExpression(u) => {
                let mut p = self.pure(&u.argument, values)?;
                p.kind = match u.operator {
                    UnaryOperator::LogicalNot => Kind::Boolean,
                    UnaryOperator::Typeof => Kind::String,
                    UnaryOperator::Void => Kind::Undefined,
                    UnaryOperator::UnaryPlus
                    | UnaryOperator::UnaryNegation
                    | UnaryOperator::BitwiseNot
                        if p.kind.coercible() =>
                    {
                        Kind::Number
                    }
                    _ => return None,
                };
                p.key = format!("({:?} {})", u.operator, p.key);
                Some(p)
            }
            Expression::BinaryExpression(b) => {
                let mut left = self.pure(&b.left, values)?;
                let right = self.pure(&b.right, values)?;
                let kind = if matches!(
                    b.operator,
                    BinaryOperator::StrictEquality | BinaryOperator::StrictInequality
                ) {
                    Kind::Boolean
                } else if left.kind.coercible()
                    && right.kind.coercible()
                    && !b.operator.is_in()
                    && !b.operator.is_instance_of()
                {
                    if b.operator.is_compare() {
                        Kind::Boolean
                    } else if b.operator == BinaryOperator::Addition
                        && (left.kind == Kind::String || right.kind == Kind::String)
                    {
                        Kind::String
                    } else {
                        Kind::Number
                    }
                } else {
                    return None;
                };
                let commutative = matches!(
                    b.operator,
                    BinaryOperator::BitwiseOR
                        | BinaryOperator::BitwiseAnd
                        | BinaryOperator::BitwiseXOR
                        | BinaryOperator::StrictEquality
                        | BinaryOperator::StrictInequality
                );
                left.key = if commutative && left.key > right.key {
                    format!("({:?} {} {})", b.operator, right.key, left.key)
                } else {
                    format!("({:?} {} {})", b.operator, left.key, right.key)
                };
                left.kind = kind;
                left.reads.extend(right.reads);
                left.versions.extend(right.versions);
                Some(left)
            }
            Expression::LogicalExpression(l) => {
                let mut left = self.pure(&l.left, values)?;
                let right = self.pure(&l.right, values)?;
                left.key = format!("({:?} {} {})", l.operator, left.key, right.key);
                if left.kind != right.kind {
                    left.kind = Kind::Unknown;
                }
                left.reads.extend(right.reads);
                left.versions.extend(right.versions);
                Some(left)
            }
            Expression::ConditionalExpression(c) => {
                let mut test = self.pure(&c.test, values)?;
                let yes = self.pure(&c.consequent, values)?;
                let no = self.pure(&c.alternate, values)?;
                test.key = format!("(?: {} {} {})", test.key, yes.key, no.key);
                test.kind = if yes.kind == no.kind {
                    yes.kind
                } else {
                    Kind::Unknown
                };
                test.reads.extend(yes.reads);
                test.reads.extend(no.reads);
                test.versions.extend(yes.versions);
                test.versions.extend(no.versions);
                Some(test)
            }
            _ => None,
        }
    }
    // Result type on successful completion, distinct from purity. For example,
    // an unknown object's multiplication can run valueOf, but a Number operand
    // guarantees that a successfully stored result is a Number.
    fn kind(&self, e: &Expression<'_>, values: &Values) -> Kind {
        if let Some(p) = self.pure(e, values) {
            return p.kind;
        }
        match e {
            Expression::ParenthesizedExpression(p) => self.kind(&p.expression, values),
            Expression::UnaryExpression(u) => match u.operator {
                UnaryOperator::UnaryPlus => Kind::Number,
                UnaryOperator::LogicalNot => Kind::Boolean,
                UnaryOperator::Typeof => Kind::String,
                UnaryOperator::Void => Kind::Undefined,
                _ => Kind::Unknown,
            },
            Expression::BinaryExpression(b)
                if b.operator.is_compare() || b.operator.is_in() || b.operator.is_instance_of() =>
            {
                Kind::Boolean
            }
            Expression::BinaryExpression(b)
                if b.operator != BinaryOperator::Addition
                    && (self.kind(&b.left, values) == Kind::Number
                        || self.kind(&b.right, values) == Kind::Number) =>
            {
                Kind::Number
            }
            _ => Kind::Unknown,
        }
    }
    fn body(&mut self, statements: &[Statement<'a>]) {
        let empty = if statements.iter().any(|s| {
            matches!(s,Statement::ExpressionStatement(s)
            if matches!(s.expression,Expression::StringLiteral(_)))
        }) {
            ";"
        } else {
            ""
        };
        let mut values = statements
            .first()
            .map_or_else(Values::new, |s| self.numeric_entry(s.span().start));
        let mut available = BTreeMap::<String, (SymbolId, u32)>::new();
        for statement in statements {
            if matches!(statement, Statement::FunctionDeclaration(_)) {
                continue;
            }
            if !matches!(
                statement,
                Statement::VariableDeclaration(_)
                    | Statement::ExpressionStatement(_)
                    | Statement::ReturnStatement(_)
                    | Statement::ThrowStatement(_)
                    | Statement::EmptyStatement(_)
            ) {
                // The actual joined value is unknown. Give it a fresh epoch
                // rather than reusing either incoming branch's value number.
                values = self.numeric_entry(statement.span().end);
                available.clear();
                continue;
            }
            let store = match statement {
                Statement::VariableDeclaration(d) if d.declarations.len() == 1 => {
                    match (&d.declarations[0].id, &d.declarations[0].init) {
                        (BindingPattern::BindingIdentifier(id), Some(init)) => {
                            id.symbol_id.get().map(|s| (s, init, true))
                        }
                        _ => None,
                    }
                }
                Statement::ExpressionStatement(s) => match &s.expression {
                    Expression::AssignmentExpression(a)
                        if a.operator == AssignmentOperator::Assign =>
                    {
                        match &a.left {
                            AssignmentTarget::AssignmentTargetIdentifier(id) => {
                                analysis::identifier_symbol(id, self.scoping)
                                    .map(|s| (s, &a.right, false))
                            }
                            _ => None,
                        }
                    }
                    _ => None,
                },
                _ => None,
            };
            let stored = store.and_then(|(_, init, _)| self.pure(init, &values));
            if let Statement::ExpressionStatement(s) = statement {
                if !matches!(s.expression, Expression::StringLiteral(_))
                    && self.pure(&s.expression, &values).is_some()
                {
                    self.edits.push(analysis::Edit::replace(s.span, empty));
                    continue;
                }
            }
            if let Some((symbol, _, false)) = store {
                if !self.scoping.symbol_flags(symbol).is_const_variable()
                    && stored
                        .as_ref()
                        .is_some_and(|p| available.get(&p.key).is_some_and(|(s, _)| *s == symbol))
                {
                    self.edits
                        .push(analysis::Edit::replace(statement.span(), empty));
                    continue;
                }
            }
            if let Some((symbol, init, true)) = store {
                if self.owned(
                    symbol,
                    Span::new(statement.span().end, statement.span().end),
                ) && !self
                    .scoping
                    .get_resolved_references(symbol)
                    .any(|r| r.is_write())
                {
                    self.definitions.insert(
                        symbol,
                        Definition {
                            init: init.span(),
                            name: self.scoping.symbol_name(symbol).into(),
                        },
                    );
                    if let Some(pure) = stored.clone() {
                        self.pure_definitions.insert(symbol, pure);
                    }
                }
            }
            let mut writes = Writes {
                scoping: self.scoping,
                symbols: BTreeSet::new(),
            };
            writes.visit_statement(statement);
            // Kill a value before visiting a statement that might write it,
            // including writes hidden in arguments, branches, or destructuring.
            for symbol in &writes.symbols {
                values.remove(symbol);
            }
            // A store inside the initializer may change an operand before it
            // is read. Do not infer its type from the statement-entry value.
            let kind = store.map(|(_, init, _)| self.kind(init, &values));
            available.retain(|_, (symbol, version)| {
                values.get(symbol).is_some_and(|v| v.version == *version)
            });
            let mut expressions = Expressions {
                scan: self,
                values: &values,
                available: &available,
            };
            expressions.visit_statement(statement);
            if let Some((symbol, _, _)) = store {
                if self.owned(
                    symbol,
                    Span::new(statement.span().end, statement.span().end),
                ) {
                    let version = statement.span().start;
                    values.insert(
                        symbol,
                        Value {
                            version,
                            kind: kind.unwrap_or(Kind::Unknown),
                        },
                    );
                    if let Some(p) = stored.filter(|p| p.compound()) {
                        // A dependency changed by the store cannot keep the
                        // expression number it had before this statement.
                        if !p.reads.iter().any(|(s, _)| writes.symbols.contains(s)) {
                            available.insert(p.key, (symbol, version));
                        }
                    }
                }
            }
        }
    }
    fn numeric_entry(&self, point: u32) -> Values {
        self.locals
            .numeric_cells(Span::new(point, point))
            .filter(|s| !self.immutable.contains_key(s))
            .filter(|&s| self.owned(s, Span::new(point, point)))
            .map(|s| {
                (
                    s,
                    Value {
                        version: point,
                        kind: Kind::NumericPrimitive,
                    },
                )
            })
            .collect()
    }
}
impl<'a> Visit<'a> for Scan<'_, 'a> {
    fn visit_function(&mut self, f: &Function<'a>, flags: ScopeFlags) {
        if let Some(b) = &f.body {
            self.body(&b.statements);
        }
        walk::walk_function(self, f, flags);
    }
    fn visit_arrow_function_expression(&mut self, f: &ArrowFunctionExpression<'a>) {
        if !f.expression {
            self.body(&f.body.statements);
        }
        walk::walk_arrow_function_expression(self, f);
    }
    fn visit_block_statement(&mut self, b: &BlockStatement<'a>) {
        self.body(&b.body);
        walk::walk_block_statement(self, b);
    }
}
struct Discover<'s, 't, 'a>(&'s mut Scan<'t, 'a>);
impl<'a> Visit<'a> for Discover<'_, '_, 'a> {
    fn visit_variable_declarator(&mut self, d: &VariableDeclarator<'a>) {
        if let (BindingPattern::BindingIdentifier(id), Some(init)) = (&d.id, &d.init) {
            if let Some(symbol) = id.symbol_id.get() {
                if self.0.regions.declarations.contains_key(&symbol)
                    && self.0.owned(
                        symbol,
                        Span::new(
                            self.0.regions.declarations[&symbol].0.end,
                            self.0.regions.declarations[&symbol].0.end,
                        ),
                    )
                    && self.0.scoping.symbol_redeclarations(symbol).is_empty()
                    && self
                        .0
                        .scoping
                        .get_resolved_references(symbol)
                        .all(|r| !r.is_write())
                {
                    let kind = self.0.kind(init, &Values::new());
                    self.0.immutable.insert(
                        symbol,
                        Value {
                            kind,
                            version: self.0.regions.declarations[&symbol].0.start,
                        },
                    );
                }
            }
        }
        walk::walk_variable_declarator(self, d);
    }
}
struct Writes<'s> {
    scoping: &'s Scoping,
    symbols: BTreeSet<SymbolId>,
}
impl<'a> Visit<'a> for Writes<'_> {
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        if let Some(r) = id.reference_id.get().map(|r| self.scoping.get_reference(r)) {
            if r.is_write() {
                if let Some(s) = r.symbol_id() {
                    self.symbols.insert(s);
                }
            }
        }
    }
    fn visit_variable_declarator(&mut self, d: &VariableDeclarator<'a>) {
        if d.init.is_some() {
            for id in d.id.get_binding_identifiers() {
                if let Some(s) = id.symbol_id.get() {
                    self.symbols.insert(s);
                }
            }
        }
        walk::walk_variable_declarator(self, d);
    }
    fn visit_function(&mut self, _: &Function<'a>, _: ScopeFlags) {}
    fn visit_arrow_function_expression(&mut self, _: &ArrowFunctionExpression<'a>) {}
}
struct Expressions<'s, 't, 'a> {
    scan: &'s mut Scan<'t, 'a>,
    values: &'s Values,
    available: &'s BTreeMap<String, (SymbolId, u32)>,
}
impl<'a> Visit<'a> for Expressions<'_, '_, 'a> {
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        let Some(symbol) = analysis::identifier_symbol(id, self.scan.scoping) else {
            return;
        };
        let Some(pure) = self.scan.pure_definitions.get(&symbol) else {
            return;
        };
        if pure.versions.iter().all(|(dep, version)| {
            self.values
                .get(dep)
                .or_else(|| {
                    self.scan
                        .immutable
                        .get(dep)
                        .filter(|_| self.scan.dominated(*dep, id.span))
                })
                .is_some_and(|v| v.version == *version)
        }) {
            self.scan.stable_uses.insert(id.span);
        }
    }
    fn visit_expression(&mut self, e: &Expression<'a>) {
        // Recurse first to record the nearest pure consumer of each reference.
        // A surrounding expression is not interchangeable with that consumer.
        walk::walk_expression(self, e);
        let Some(p) = self.scan.pure(e, self.values) else {
            return;
        };
        if !p.compound() {
            return;
        }
        if p.reads.len() == 1 {
            let (symbol, read) = p.reads[0];
            self.scan
                .contexts
                .entry(symbol)
                .or_default()
                .entry(read.start)
                .or_insert_with(|| Context {
                    span: e.span(),
                    read,
                    key: p.key.clone(),
                });
        }
        if let Some((symbol, _)) = self.available.get(&p.key) {
            let resolves = p.reads.first().is_some_and(|(source, read)| {
                self.scan.scoping.get_resolved_references(*source).any(|r| {
                    self.scan.nodes.get_node(r.node_id()).kind().span() == *read
                        && self
                            .scan
                            .scoping
                            .find_binding(r.scope_id(), self.scan.scoping.symbol_ident(*symbol))
                            == Some(*symbol)
                })
            });
            if resolves {
                self.scan.edits.push(analysis::Edit::replace(
                    e.span(),
                    self.scan.scoping.symbol_name(*symbol),
                ));
            }
        }
    }
    fn visit_function(&mut self, _: &Function<'a>, _: ScopeFlags) {}
    fn visit_arrow_function_expression(&mut self, _: &ArrowFunctionExpression<'a>) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn joined_numeric_cells_use_fresh_versions_for_pure_snapshots() {
        let source="function f(flag){let log=[];let cell=0;if(flag){cell=2;}else{cell=3;}let $SSA_calc=cell*4;log.push('gap');let value=$SSA_calc+1;cell=9;return[log,value,cell];}";
        let code = clean(source);
        assert!(!code.contains("$SSA_calc"), "{code}");
        assert_eq!(run(source), run(&code));
    }
    fn clean(source: &str) -> String {
        let mut code = source.to_string();
        for _ in 0..12 {
            let (next, changed) = simplify_local_values(&code, &BTreeSet::new()).unwrap();
            code = next;
            if changed == 0 {
                break;
            }
        }
        code
    }
    fn run(source: &str) -> String {
        let output = std::process::Command::new("node")
            .args([
                "-e",
                &format!("{source}; console.log(JSON.stringify(f()));"),
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    }
    #[test]
    fn fuses_arithmetic_boolean_and_comparison_consumers_without_name_patterns() {
        for consumer in [
            "input + 7",
            "input & 255",
            "input >>> 2",
            "~input",
            "!input",
            "typeof input",
            "input === 17",
            "input ? 7 : 9",
            "(input + 7) * 3",
        ] {
            let source=format!("function f(){{let events=[];function read(){{events.push(3);return 3;}}let input=read()*2;let result=[{consumer},{consumer}];return [result,events];}}");
            let code = clean(&source);
            assert_eq!(run(&source), run(&code), "{code}");
            assert!(!code.contains(consumer), "consumer not fused: {code}");
            assert_eq!(code.matches("read()").count(), 2, "{code}");
        }
    }
    #[test]
    fn reuses_general_expressions_and_invalidates_each_reaching_store() {
        for source in [
            "function f(){let left=3;let right=5;let cached=left+right;return [cached,left+right];}",
            "function f(){let left=3;let right=5;let cached=left+right;left=9;return [cached,left+right];}",
            "function f(){let left=3;let right=5;let cached=left+right;cached=99;return [cached,left+right];}",
            "function f(){let left=3;let right=5;let cached=left+right;[left]=[9];return [cached,left+right];}",
            "function f(){let left=3;function change(){left=9;}let right=5;let cached=left+right;change();return [cached,left+right];}",
            "function f(){let left=3;let right=5;let cached=left+right;return [cached,{get x(){return left+right;}}.x];}",
        ] {
            let code=clean(source);
            assert_eq!(run(source),run(&code),"{code}");
        }
        let code=clean("function f(){let left=3;let right=5;let cached=left+right;return [cached,left+right];}");
        assert!(!code.contains("cached"), "{code}");
    }
    #[test]
    fn retains_getters_coercion_hooks_raw_observations_and_captures() {
        for source in [
            "function f(){let log=[];let input={valueOf(){log.push(1);return log.length;}};return [input+7,input+7,log];}",
            "function f(){let log=[];let obj={get value(){log.push(1);return log.length;}};let cached=obj.value;return [cached,obj.value,log];}",
            "function f(){let input=3.5;return [input,input|0,input|0];}",
            "function f(){let input=3.5;function read(){return input;}return [input|0,input|0,read()];}",
            "function f(){let input=3.5;return [(input|0)+(input|0),input|0];}",
            "function f(){let input=3.5;if(true){return [input|0,input|0];}}",
            "function f(){let input=3.5;eval('input=7.5');return [input|0,input|0];}",
            "function f(){try{let result=typeof missing;let missing=3;return result;}catch(e){return e.name;}}",
            "function f(){let input=1;let output=(input=2n)*input;let log=[];log.push('before');try{return [output|0,log];}catch(e){return [e.name,log];}}",
        ] {let code=clean(source);assert_eq!(run(source),run(&code),"{code}");}
    }
}
