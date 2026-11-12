//! The slot-local property census: the host members, calls, string
//! literals, `for…in` walks, path tables, sorted name lists and frame realm
//! of the code that feeds one slot's own emissions ([`Census`]), built from
//! one program-wide [`ProgramIndex`].
//!
//! Census probes live in grouped producers shared by five slots and most
//! write one hash, count or flag byte, so the emission shape alone never
//! decides; recognizers decide by the host object the probe enumerates or
//! tests (`getComputedStyle`, `Math`, `console`, `navigator.brave`, …).
//!
//! Used by: `AnchorIdnaUnicodeVersion`, `AnchorProperties`,
//! `BraveDuckDuckGoNavigatorFlags`, `ComputedStyleProperties`,
//! `ConsoleMemberNativenessHashPair`, `ConsolePropertyNamesHash`,
//! `CrossOriginIsolationFlags`, `GlobalOwnPropertyBits`,
//! `GlobalPropertiesCount`, `GlobalPropertyChecks`,
//! `IframeRealmPropertyNameHashes`, `LocationPropertyNames`,
//! `MathOwnPropertyNames`, `NamespacePropertyNameHashes`,
//! `NestedFrameAnchorPropertyNamesHash`, `SelectMetaInjectionFlags`,
//! `SeleniumIdeArtifactFlags`, `StorageAccessApiPresence`,
//! `WebAuthnInterfacePresence`; helper modules
//! [`census_probes`](super::census_probes).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::rc::Rc;

use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{Scoping, SymbolId};
use oxc_span::GetSpan;
use oxc_syntax::scope::ScopeFlags;

use super::{FinderCtx, SlotView};

/// A source range `(start, end)`.
pub(crate) type Range = (u32, u32);

/// `offset` lies in `range` (end exclusive).
pub(crate) fn inside(offset: u32, range: Range) -> bool {
    offset >= range.0 && offset < range.1
}

/// `outer` contains `inner`.
pub(crate) fn contains(outer: Range, inner: Range) -> bool {
    inner.0 >= outer.0 && inner.1 <= outer.1
}

/// One indexed fact at a source offset (member read, call, string, `for…in`, path table, …).
#[derive(Clone, Debug)]
pub(crate) enum Fact {
    Member(String),
    Call { method: String, args: Vec<String>, construct: bool },
    Global(String),
    Str(String),
    Regex(String),
    ForIn,
    /// `T[k]` read with `k` the key variable of an enclosing `for…in` (a
    /// table lookup per enumerated key).
    KeyLookup,
    /// An array literal of arrays of string literals (a path table).
    PathTable(Vec<Vec<String>>),
}

/// The right-hand side of a recorded write.
#[derive(Clone, Debug)]
pub(crate) enum Rhs {
    Function(Range),
    Ident(SymbolId),
    Other,
}

/// One write of a binding: where, in which function, and what was assigned.
#[derive(Clone, Debug)]
pub(crate) struct Write {
    pub(crate) offset: u32,
    pub(crate) func: usize,
    pub(crate) rhs: Rhs,
}

/// The head of a control statement (`if` test, loop head) and its statement.
#[derive(Clone, Debug)]
pub(crate) struct Head {
    /// The owning control statement.
    pub(crate) stmt: Range,
    pub(crate) head: Range,
    pub(crate) func: usize,
}

/// One call's argument facts used by binders.
#[derive(Clone, Debug, Default)]
pub(crate) struct CallArgs {
    pub(crate) end: u32,
    /// `x[N]` arguments with a numeric literal index.
    pub(crate) indices: Vec<u32>,
    /// Identifiers `L` of `L.sort()` calls inside the arguments.
    pub(crate) sorted: Vec<SymbolId>,
}

/// Program-wide facts, writes, functions and statement units, sorted by offset.
#[derive(Default)]
pub(crate) struct ProgramIndex {
    pub(crate) functions: Vec<Range>,
    /// Enclosing function of each function.
    pub(crate) parents: Vec<usize>,
    /// `S.push(..)` / `S[k] = ..` / `S.k = ..` sites of each symbol.
    pub(crate) mutations: HashMap<SymbolId, Vec<u32>>,
    /// Offsets of assignment-target identifiers (a write, not a read).
    pub(crate) write_targets: std::collections::HashSet<u32>,
    /// Simple statements and heads, with their innermost function.
    pub(crate) units: Vec<(Range, usize)>,
    pub(crate) stmts: Vec<Range>,
    pub(crate) heads: Vec<Head>,
    pub(crate) refs: Vec<(u32, SymbolId, usize)>,
    pub(crate) writes: HashMap<SymbolId, Vec<Write>>,
    pub(crate) fn_decl: HashMap<SymbolId, Range>,
    /// Parameter symbol -> callee symbol of the call its function is an
    /// argument of.
    pub(crate) param_callee: HashMap<SymbolId, SymbolId>,
    pub(crate) facts: Vec<(u32, Fact)>,
    /// `observer.next([..])` / `observer.next({..})` sites.
    pub(crate) emissions: Vec<(u32, SymbolId)>,
    /// The buffer identifier of `observer.next([callbacks, buffer])`.
    pub(crate) emission_buffer: HashMap<u32, SymbolId>,
    pub(crate) call_args: BTreeMap<u32, CallArgs>,
    /// `L.push(...)` sites under a native-code test: (offset, list, native).
    pub(crate) pushes: Vec<(u32, SymbolId, bool)>,
    /// Calls of a local identifier callee: (offset, callee), in source order.
    pub(crate) local_calls: Vec<(u32, SymbolId)>,
}

/// The AST visitor that fills a [`ProgramIndex`].
pub(crate) struct Indexer<'s> {
    pub(crate) scoping: &'s Scoping,
    pub(crate) index: ProgramIndex,
    pub(crate) func_stack: Vec<usize>,
    pub(crate) arg_fn_callee: HashMap<u32, SymbolId>,
    /// Native-code test polarity of enclosing if branches.
    pub(crate) native_ctx: Vec<bool>,
    /// The string path last assigned to a variable.
    pub(crate) paths: HashMap<SymbolId, Vec<String>>,
    /// Key variables of `for…in` loops.
    pub(crate) for_in_keys: std::collections::HashSet<SymbolId>,
    /// Method pairs `x = [o, o.m]` (the tracer's method extraction): x -> m.
    pub(crate) method_pairs: HashMap<SymbolId, (String, Option<SymbolId>)>,
    /// `s = L.sort()` declarations: s -> L.
    pub(crate) sorted_aliases: HashMap<SymbolId, SymbolId>,
    /// Variables assigned a native-code test (`t = !/native code/.test(f)`),
    /// with the test's polarity, for a later `if (t)`. A boolean literal
    /// arm (`else t = false`) or any assignment in the other arm of the
    /// `if` that holds the test assignment keeps the entry.
    pub(crate) test_vars: HashMap<SymbolId, (bool, Option<(u32, bool)>)>,
    /// Enclosing `if` arms, innermost last: (if start, in the alternate).
    pub(crate) if_arms: Vec<(u32, bool)>,
    /// Statement lists being walked, innermost last (a fresh id per list).
    pub(crate) stmt_lists: Vec<u32>,
    pub(crate) next_list: u32,
    /// `a = x[N]` statements of a statement list: a -> (N, list id, killed
    /// from this offset on by a later write of `a`).
    pub(crate) index_aliases: HashMap<SymbolId, (u32, u32, Option<u32>)>,
    /// Identifier targets of plain `=` assignments: their kill takes effect
    /// after the assigned value is evaluated.
    pub(crate) plain_targets: std::collections::HashSet<u32>,
}

/// The [`Range`] of a span.
pub(crate) fn range(span: oxc_span::Span) -> Range {
    (span.start, span.end)
}

/// The binding an identifier expression resolves to.
pub(crate) fn ident_symbol(expression: &Expression<'_>, scoping: &Scoping) -> Option<SymbolId> {
    match expression.without_parentheses() {
        Expression::Identifier(identifier) => crate::analysis::identifier_symbol(identifier, scoping),
        _ => None,
    }
}

/// The method or function name a callee denotes (`o.m`, `m`, `o["m"]`).
pub(crate) fn callee_method(callee: &Expression<'_>) -> Option<String> {
    match callee.without_parentheses() {
        Expression::Identifier(identifier) => Some(identifier.name.to_string()),
        Expression::StaticMemberExpression(member) => Some(member.property.name.to_string()),
        Expression::ComputedMemberExpression(member) => match &member.expression {
            Expression::StringLiteral(literal) => Some(literal.value.to_string()),
            _ => None,
        },
        _ => None,
    }
}

/// `[o, o.m]` (`o` an identifier or a static member chain): the method
/// name `m`.
pub(crate) fn method_pair(array: &ArrayExpression<'_>) -> Option<String> {
    let [first, second] = array.elements.as_slice() else {
        return None;
    };
    let (Some(receiver), Some(Expression::StaticMemberExpression(method))) =
        (first.as_expression(), second.as_expression())
    else {
        return None;
    };
    same_chain(&method.object, receiver).then(|| method.property.name.to_string())
}

/// Two static member chains spell the same path (`a.b.c`).
pub(crate) fn same_chain(left: &Expression<'_>, right: &Expression<'_>) -> bool {
    match (left.without_parentheses(), right.without_parentheses()) {
        (Expression::Identifier(a), Expression::Identifier(b)) => a.name == b.name,
        (Expression::StaticMemberExpression(a), Expression::StaticMemberExpression(b)) => {
            a.property.name == b.property.name && same_chain(&a.object, &b.object)
        }
        (Expression::ThisExpression(_), Expression::ThisExpression(_)) => true,
        _ => false,
    }
}

/// The string-literal arguments of a call.
pub(crate) fn string_arguments(arguments: &[Argument<'_>]) -> Vec<String> {
    arguments
        .iter()
        .filter_map(|argument| match argument {
            Argument::StringLiteral(literal) => Some(literal.value.to_string()),
            _ => None,
        })
        .collect()
}

/// `re.test(...)` (regex literal or variable receiver) inside `expression`,
/// with the number of logical negations wrapping it.
pub(crate) fn native_test_polarity(expression: &Expression<'_>, negations: usize) -> Option<bool> {
    match expression.without_parentheses() {
        Expression::UnaryExpression(unary) if unary.operator == UnaryOperator::LogicalNot => {
            native_test_polarity(&unary.argument, negations + 1)
        }
        Expression::LogicalExpression(logical) => native_test_polarity(&logical.left, negations)
            .or_else(|| native_test_polarity(&logical.right, negations)),
        Expression::CallExpression(call) => match call.callee.without_parentheses() {
            Expression::StaticMemberExpression(member) if member.property.name == "test" => {
                Some(negations % 2 == 1)
            }
            _ => None,
        },
        _ => None,
    }
}

/// An array literal of string literals (`["Navigator", "prototype"]`).
pub(crate) fn string_path(array: &ArrayExpression<'_>) -> Option<Vec<String>> {
    let mut path = Vec::new();
    for step in &array.elements {
        let ArrayExpressionElement::StringLiteral(literal) = step else {
            return None;
        };
        path.push(literal.value.to_string());
    }
    (!path.is_empty()).then_some(path)
}

/// An array literal of paths; an element may be a variable last assigned a
/// string path (`let p = ["Navigator", "prototype"]; [p, ["safari"]]`).
pub(crate) fn path_table(
    array: &ArrayExpression<'_>,
    scoping: &Scoping,
    paths: &HashMap<SymbolId, Vec<String>>,
) -> Option<Vec<Vec<String>>> {
    if array.elements.len() < 2 {
        return None;
    }
    let mut table = Vec::new();
    for element in &array.elements {
        let path = match element {
            ArrayExpressionElement::ArrayExpression(inner) => string_path(inner)?,
            ArrayExpressionElement::Identifier(identifier) => {
                paths.get(&crate::analysis::identifier_symbol(identifier, scoping)?)?.clone()
            }
            _ => return None,
        };
        table.push(path);
    }
    Some(table)
}

impl Indexer<'_> {
    /// The tracer's register copy of a `for…in` key at the head of the loop
    /// body (`for (const k in o) { r = k; let c = table[r]; … }`): a lookup
    /// declared right after the copy, before anything else runs, indexes
    /// the table by the key ([`Fact::KeyLookup`]). The register is reused
    /// later in the body, so only these leading lookups count.
    fn key_copy_lookups(&mut self, key: SymbolId, body: &Statement<'_>) {
        let Statement::BlockStatement(block) = body else {
            return;
        };
        let mut copies: Vec<SymbolId> = vec![key];
        for statement in &block.body {
            match statement {
                Statement::ExpressionStatement(expression) => {
                    let Expression::AssignmentExpression(assignment) = expression.expression.without_parentheses() else {
                        return;
                    };
                    let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else {
                        return;
                    };
                    let copied = assignment.operator.is_assign()
                        && ident_symbol(&assignment.right, self.scoping).is_some_and(|symbol| copies.contains(&symbol));
                    let Some(register) = crate::analysis::identifier_symbol(target, self.scoping).filter(|_| copied) else {
                        return;
                    };
                    copies.push(register);
                }
                Statement::VariableDeclaration(declaration) => {
                    for declarator in &declaration.declarations {
                        let Some(Expression::ComputedMemberExpression(member)) =
                            declarator.init.as_ref().map(|init| init.without_parentheses())
                        else {
                            return;
                        };
                        if !ident_symbol(&member.expression, self.scoping).is_some_and(|symbol| copies[1..].contains(&symbol)) {
                            return;
                        }
                        self.fact(member.span.start, Fact::KeyLookup);
                    }
                }
                _ => return,
            }
        }
    }

    /// Record or clear `symbol`'s native-test polarity for an assignment.
    pub(crate) fn note_test_var(&mut self, symbol: SymbolId, rhs: &Expression<'_>) {
        let arm = self.if_arms.last().copied();
        match native_test_polarity(rhs, 0) {
            Some(negated) => {
                self.test_vars.insert(symbol, (negated, arm));
            }
            None if matches!(rhs.without_parentheses(), Expression::BooleanLiteral(_)) => {}
            None => {
                let sibling = matches!((self.test_vars.get(&symbol), arm),
                    (Some((_, Some((at, alternate)))), Some((here, here_alternate))) if *at == here && *alternate != here_alternate);
                if !sibling {
                    self.test_vars.remove(&symbol);
                }
            }
        }
    }

    /// `if (t)` / `if (!t)` with `t` last assigned a native-code test.
    pub(crate) fn test_var_polarity(&self, test: &Expression<'_>, negations: usize) -> Option<bool> {
        match test.without_parentheses() {
            Expression::UnaryExpression(unary) if unary.operator == UnaryOperator::LogicalNot => {
                self.test_var_polarity(&unary.argument, negations + 1)
            }
            Expression::Identifier(identifier) => {
                let symbol = crate::analysis::identifier_symbol(identifier, self.scoping)?;
                self.test_vars.get(&symbol).map(|(negated, _)| (*negated) ^ (negations % 2 == 1))
            }
            _ => None,
        }
    }

    /// The method `m` a `.call` callee calls, with the receiver variable
    /// when known: `x[1].call` with `x = [o, o.m]` (receiver `o`), or
    /// `o.m.call(r, ..)` (receiver `r`).
    pub(crate) fn call_through(&self, call_expression: &CallExpression<'_>) -> Option<(String, Option<SymbolId>)> {
        let callee = &call_expression.callee;
        let Expression::StaticMemberExpression(call) = callee.without_parentheses() else {
            return None;
        };
        if call.property.name != "call" {
            return None;
        }
        match call.object.without_parentheses() {
            Expression::ComputedMemberExpression(member) => {
                let Expression::NumericLiteral(index) = &member.expression else {
                    return None;
                };
                if index.value != 1.0 {
                    return None;
                }
                let pair = ident_symbol(&member.object, self.scoping)?;
                self.method_pairs.get(&pair).cloned()
            }
            Expression::StaticMemberExpression(member) => {
                let receiver = call_expression
                    .arguments
                    .first()
                    .and_then(Argument::as_expression)
                    .and_then(|receiver| ident_symbol(receiver, self.scoping));
                Some((member.property.name.to_string(), receiver))
            }
            _ => None,
        }
    }

    /// The index of the innermost enclosing function (`usize::MAX` at top level).
    pub(crate) fn func(&self) -> usize {
        self.func_stack.last().copied().unwrap_or(usize::MAX)
    }

    /// Record a fact at `offset`; generated spellings are dropped.
    pub(crate) fn fact(&mut self, offset: u32, fact: Fact) {
        // Generated spellings are never facts.
        let fact = match fact {
            Fact::Member(name) | Fact::Global(name) if name.starts_with('$') => return,
            // A call of a generated callee keeps its string arguments only.
            Fact::Call { method, args, construct } if method.starts_with('$') => Fact::Call {
                method: String::new(),
                args,
                construct,
            },
            fact => fact,
        };
        self.index.facts.push((offset, fact));
    }

    /// Record a write of `symbol` at `offset`.
    pub(crate) fn write(&mut self, symbol: SymbolId, offset: u32, rhs: Rhs) {
        let func = self.func();
        self.index.writes.entry(symbol).or_default().push(Write { offset, func, rhs });
        self.index.write_targets.insert(offset);
    }

    /// What an assigned expression is: a function, an alias of a binding, or other.
    pub(crate) fn rhs(&self, expression: &Expression<'_>) -> Rhs {
        match expression.without_parentheses() {
            Expression::FunctionExpression(function) => Rhs::Function(range(function.span)),
            Expression::ArrowFunctionExpression(arrow) => Rhs::Function(range(arrow.span)),
            other => ident_symbol(other, self.scoping).map_or(Rhs::Other, Rhs::Ident),
        }
    }

    /// Open a function scope; parameters of a callback remember its callee.
    pub(crate) fn enter_function(&mut self, span: oxc_span::Span, params: &FormalParameters<'_>) {
        let id = self.index.functions.len();
        let parent = self.func();
        self.index.functions.push(range(span));
        self.index.parents.push(parent);
        self.func_stack.push(id);
        if let Some(callee) = self.arg_fn_callee.get(&span.start).copied() {
            for param in &params.items {
                if let BindingPattern::BindingIdentifier(binding) = &param.pattern {
                    if let Some(symbol) = binding.symbol_id.get() {
                        self.index.param_callee.insert(symbol, callee);
                    }
                }
            }
        }
    }

    /// Record the head of a control statement as its own unit.
    pub(crate) fn head(&mut self, stmt: oxc_span::Span, head: Range) {
        let func = self.func();
        self.index.heads.push(Head { stmt: range(stmt), head, func });
        self.index.units.push((head, func));
    }
}

/// `x[N]` with a non-negative integer literal index.
fn literal_index(expression: &Expression<'_>) -> Option<u32> {
    let Expression::ComputedMemberExpression(member) = expression.without_parentheses() else {
        return None;
    };
    let Expression::NumericLiteral(literal) = &member.expression else {
        return None;
    };
    (literal.value.fract() == 0.0 && literal.value >= 0.0).then_some(literal.value as u32)
}

impl Indexer<'_> {
    /// A later write of `symbol` ends its `a = x[N]` alias from `offset` on.
    fn kill_index_alias(&mut self, symbol: SymbolId, offset: u32) {
        if let Some((_, _, kill)) = self.index_aliases.get_mut(&symbol) {
            *kill = Some(kill.map_or(offset, |old| old.min(offset)));
        }
    }

    /// The tuple index an identifier argument holds at `offset`: the last
    /// `a = x[N]` statement of the same statement list, with no write of
    /// `a` since.
    fn aliased_index(&self, expression: &Expression<'_>, offset: u32) -> Option<u32> {
        let symbol = ident_symbol(expression, self.scoping)?;
        let (index, list, kill) = self.index_aliases.get(&symbol)?;
        (Some(list) == self.stmt_lists.last() && kill.is_none_or(|kill| offset < kill)).then_some(*index)
    }
}

impl<'a> Visit<'a> for Indexer<'_> {
    fn visit_statements(&mut self, statements: &oxc_allocator::Vec<'a, Statement<'a>>) {
        let list = self.next_list;
        self.next_list += 1;
        self.stmt_lists.push(list);
        for statement in statements {
            self.visit_statement(statement);
            // `a = x[N];` / `let a = x[N];` straight in this list.
            let mut aliases: Vec<(SymbolId, u32)> = Vec::new();
            match statement {
                Statement::ExpressionStatement(statement) => {
                    if let Expression::AssignmentExpression(assignment) = statement.expression.without_parentheses() {
                        if let (AssignmentOperator::Assign, AssignmentTarget::AssignmentTargetIdentifier(target)) =
                            (assignment.operator, &assignment.left)
                        {
                            if let (Some(symbol), Some(index)) = (
                                crate::analysis::identifier_symbol(target, self.scoping),
                                literal_index(&assignment.right),
                            ) {
                                aliases.push((symbol, index));
                            }
                        }
                    }
                }
                Statement::VariableDeclaration(declaration) => {
                    for declarator in &declaration.declarations {
                        if let (BindingPattern::BindingIdentifier(binding), Some(init)) =
                            (&declarator.id, declarator.init.as_ref())
                        {
                            if let (Some(symbol), Some(index)) = (binding.symbol_id.get(), literal_index(init)) {
                                aliases.push((symbol, index));
                            }
                        }
                    }
                }
                _ => {}
            }
            for (symbol, index) in aliases {
                self.index_aliases.insert(symbol, (index, list, None));
            }
        }
        self.stmt_lists.pop();
    }

    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        if let Some(symbol) = function.id.as_ref().and_then(|id| id.symbol_id.get()) {
            self.index.fn_decl.insert(symbol, range(function.span));
        }
        self.enter_function(function.span, &function.params);
        let saved = std::mem::take(&mut self.native_ctx);
        let saved_arms = std::mem::take(&mut self.if_arms);
        walk::walk_function(self, function, flags);
        self.if_arms = saved_arms;
        self.native_ctx = saved;
        self.func_stack.pop();
    }

    fn visit_arrow_function_expression(&mut self, arrow: &ArrowFunctionExpression<'a>) {
        self.enter_function(arrow.span, &arrow.params);
        let saved = std::mem::take(&mut self.native_ctx);
        let saved_arms = std::mem::take(&mut self.if_arms);
        walk::walk_arrow_function_expression(self, arrow);
        self.if_arms = saved_arms;
        self.native_ctx = saved;
        self.func_stack.pop();
    }

    fn visit_statement(&mut self, statement: &Statement<'a>) {
        let span = statement.span();
        self.index.stmts.push(range(span));
        match statement {
            Statement::ExpressionStatement(_)
            | Statement::VariableDeclaration(_)
            | Statement::ReturnStatement(_)
            | Statement::ThrowStatement(_) => {
                let func = self.func();
                self.index.units.push((range(span), func));
            }
            Statement::WhileStatement(loop_) => self.head(span, range(loop_.test.span())),
            Statement::DoWhileStatement(loop_) => self.head(span, range(loop_.test.span())),
            Statement::ForStatement(loop_) => {
                let end = loop_.body.span().start;
                self.head(span, (span.start, end));
            }
            Statement::ForInStatement(loop_) => {
                self.head(span, (span.start, loop_.right.span().end));
                self.fact(span.start, Fact::ForIn);
                let key = match &loop_.left {
                    ForStatementLeft::VariableDeclaration(declaration) => {
                        declaration.declarations.first().and_then(|declarator| match &declarator.id {
                            BindingPattern::BindingIdentifier(binding) => binding.symbol_id.get(),
                            _ => None,
                        })
                    }
                    ForStatementLeft::AssignmentTargetIdentifier(target) => {
                        crate::analysis::identifier_symbol(target, self.scoping)
                    }
                    _ => None,
                };
                if let Some(key) = key {
                    self.for_in_keys.insert(key);
                    self.key_copy_lookups(key, &loop_.body);
                }
            }
            Statement::ForOfStatement(loop_) => self.head(span, (span.start, loop_.right.span().end)),
            Statement::SwitchStatement(switch) => self.head(span, range(switch.discriminant.span())),
            _ => {}
        }
        walk::walk_statement(self, statement);
    }

    fn visit_if_statement(&mut self, statement: &IfStatement<'a>) {
        self.head(statement.span, range(statement.test.span()));
        self.visit_expression(&statement.test);
        let polarity = native_test_polarity(&statement.test, 0).or_else(|| self.test_var_polarity(&statement.test, 0));
        // then-branch runs when the test holds: a negated native test means
        // the then-branch sees non-native values.
        if let Some(negated) = polarity {
            self.native_ctx.push(!negated);
        }
        self.if_arms.push((statement.span.start, false));
        self.visit_statement(&statement.consequent);
        self.if_arms.pop();
        if polarity.is_some() {
            self.native_ctx.pop();
        }
        if let Some(alternate) = &statement.alternate {
            if let Some(negated) = polarity {
                self.native_ctx.push(negated);
            }
            self.if_arms.push((statement.span.start, true));
            self.visit_statement(alternate);
            self.if_arms.pop();
            if polarity.is_some() {
                self.native_ctx.pop();
            }
        }
    }

    fn visit_variable_declarator(&mut self, declarator: &VariableDeclarator<'a>) {
        if let BindingPattern::BindingIdentifier(binding) = &declarator.id {
            if let Some(symbol) = binding.symbol_id.get() {
                match declarator.init.as_ref() {
                    Some(init) => self.note_test_var(symbol, init),
                    None => {
                        self.test_vars.remove(&symbol);
                    }
                }
                let rhs = declarator.init.as_ref().map_or(Rhs::Other, |init| self.rhs(init));
                self.write(symbol, binding.span.start, rhs);
                if let Some(Expression::CallExpression(call)) = declarator.init.as_ref().map(|e| e.without_parentheses()) {
                    if let Expression::StaticMemberExpression(member) = call.callee.without_parentheses() {
                        if member.property.name == "sort" && call.arguments.is_empty() {
                            if let Some(list) = ident_symbol(&member.object, self.scoping) {
                                self.sorted_aliases.insert(symbol, list);
                            }
                        }
                    }
                }
                if let Some(Expression::ArrayExpression(array)) = declarator.init.as_ref() {
                    if let Some(path) = string_path(array) {
                        self.paths.insert(symbol, path);
                    }
                    if let Some(method) = method_pair(array) {
                        let receiver = array
                            .elements
                            .first()
                            .and_then(ArrayExpressionElement::as_expression)
                            .and_then(|receiver| ident_symbol(receiver, self.scoping));
                        self.method_pairs.insert(symbol, (method, receiver));
                    }
                }
            }
        }
        if let BindingPattern::BindingIdentifier(binding) = &declarator.id {
            if let Some(symbol) = binding.symbol_id.get() {
                self.kill_index_alias(symbol, declarator.span.end);
            }
        }
        // `let [a, b] = [x, y]` (the tracer's parallel binding): each name is
        // written by the declaration.
        if let BindingPattern::ArrayPattern(pattern) = &declarator.id {
            let values = match declarator.init.as_ref().map(|init| init.without_parentheses()) {
                Some(Expression::ArrayExpression(array)) => Some(array),
                _ => None,
            };
            for (position, element) in pattern.elements.iter().enumerate() {
                let Some(BindingPattern::BindingIdentifier(binding)) = element else {
                    continue;
                };
                let Some(symbol) = binding.symbol_id.get() else {
                    continue;
                };
                let rhs = values
                    .and_then(|array| array.elements.get(position))
                    .and_then(ArrayExpressionElement::as_expression)
                    .map_or(Rhs::Other, |value| self.rhs(value));
                self.write(symbol, binding.span.start, rhs);
            }
        }
        walk::walk_variable_declarator(self, declarator);
    }

    fn visit_assignment_expression(&mut self, assignment: &AssignmentExpression<'a>) {
        match &assignment.left {
            AssignmentTarget::AssignmentTargetIdentifier(target) => {
                if let Some(symbol) = crate::analysis::identifier_symbol(target, self.scoping) {
                    if assignment.operator == AssignmentOperator::Assign {
                        self.plain_targets.insert(target.span.start);
                        self.kill_index_alias(symbol, assignment.span.end);
                    }
                    self.note_test_var(symbol, &assignment.right);
                    let rhs = self.rhs(&assignment.right);
                    self.write(symbol, target.span.start, rhs);
                }
            }
            AssignmentTarget::ComputedMemberExpression(member) => {
                if let Some(symbol) = ident_symbol(&member.object, self.scoping) {
                    self.index.mutations.entry(symbol).or_default().push(member.span.start);
                }
            }
            AssignmentTarget::StaticMemberExpression(member) => {
                if let Some(symbol) = ident_symbol(&member.object, self.scoping) {
                    self.index.mutations.entry(symbol).or_default().push(member.span.start);
                }
            }
            _ => {}
        }
        walk::walk_assignment_expression(self, assignment);
    }

    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        match crate::analysis::identifier_symbol(identifier, self.scoping) {
            Some(symbol) => {
                let write = identifier
                    .reference_id
                    .get()
                    .is_some_and(|reference| self.scoping.get_reference(reference).is_write());
                if write && !self.plain_targets.contains(&identifier.span.start) {
                    self.kill_index_alias(symbol, identifier.span.start);
                }
                let func = self.func();
                self.index.refs.push((identifier.span.start, symbol, func));
            }
            None => self.fact(identifier.span.start, Fact::Global(identifier.name.to_string())),
        }
    }

    fn visit_static_member_expression(&mut self, member: &StaticMemberExpression<'a>) {
        self.fact(member.span.start, Fact::Member(member.property.name.to_string()));
        walk::walk_static_member_expression(self, member);
    }

    fn visit_computed_member_expression(&mut self, member: &ComputedMemberExpression<'a>) {
        if let Expression::StringLiteral(literal) = &member.expression {
            self.fact(member.span.start, Fact::Member(literal.value.to_string()));
        }
        if let Some(key) = ident_symbol(&member.expression, self.scoping) {
            if self.for_in_keys.contains(&key) {
                self.fact(member.span.start, Fact::KeyLookup);
            }
        }
        walk::walk_computed_member_expression(self, member);
    }

    fn visit_string_literal(&mut self, literal: &StringLiteral<'a>) {
        self.fact(literal.span.start, Fact::Str(literal.value.to_string()));
    }

    fn visit_reg_exp_literal(&mut self, literal: &RegExpLiteral<'a>) {
        self.fact(literal.span.start, Fact::Regex(literal.regex.pattern.text.to_string()));
    }

    fn visit_array_expression(&mut self, array: &ArrayExpression<'a>) {
        if let Some(table) = path_table(array, self.scoping, &self.paths) {
            self.fact(array.span.start, Fact::PathTable(table));
        }
        walk::walk_array_expression(self, array);
    }

    fn visit_new_expression(&mut self, new: &NewExpression<'a>) {
        if let Some(method) = callee_method(&new.callee) {
            let args = string_arguments(&new.arguments);
            self.fact(new.span.start, Fact::Call { method, args, construct: true });
        }
        walk::walk_new_expression(self, new);
    }

    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if let Some(method) = callee_method(&call.callee) {
            let args = string_arguments(&call.arguments);
            self.fact(call.span.start, Fact::Call { method, args, construct: false });
        }
        // A receiver-explicit call is also the method's own call:
        // `x[1].call(x[0], ..)` with `x = [o, o.m]` (the tracer's method
        // extraction) and `o.m.call(r, ..)` both call `m`.
        if let Some((method, receiver)) = self.call_through(call) {
            let args = string_arguments(call.arguments.get(1..).unwrap_or_default());
            if let Some(receiver) = receiver {
                if matches!(method.as_str(), "push" | "unshift" | "splice") {
                    self.index.mutations.entry(receiver).or_default().push(call.span.start);
                }
                if method == "push" {
                    if let Some(native) = self.native_ctx.last().copied() {
                        self.index.pushes.push((call.span.start, receiver, native));
                    }
                }
            }
            self.fact(call.span.start, Fact::Call { method, args, construct: false });
        }
        if let Some(callee) = ident_symbol(&call.callee, self.scoping) {
            self.index.local_calls.push((call.span.start, callee));
            for argument in &call.arguments {
                match argument {
                    Argument::ArrowFunctionExpression(arrow) => {
                        self.arg_fn_callee.insert(arrow.span.start, callee);
                    }
                    Argument::FunctionExpression(function) => {
                        self.arg_fn_callee.insert(function.span.start, callee);
                    }
                    _ => {}
                }
            }
        }
        if let Expression::StaticMemberExpression(member) = call.callee.without_parentheses() {
            let object = ident_symbol(&member.object, self.scoping);
            if member.property.name == "next" {
                // `observer.next([callbacks, buffer])` or `observer.next({...})`.
                let emits = match call.arguments.first() {
                    Some(Argument::ArrayExpression(array)) => array.elements.len() == 2,
                    Some(Argument::ObjectExpression(_)) => true,
                    _ => false,
                };
                if let (true, Some(object)) = (emits, object) {
                    self.index.emissions.push((call.span.start, object));
                    if let Some(Argument::ArrayExpression(array)) = call.arguments.first() {
                        if let Some(ArrayExpressionElement::Identifier(buffer)) = array.elements.get(1) {
                            if let Some(symbol) = crate::analysis::identifier_symbol(buffer, self.scoping) {
                                self.index.emission_buffer.insert(call.span.start, symbol);
                            }
                        }
                    }
                }
            }
            if matches!(member.property.name.as_str(), "push" | "unshift" | "splice") {
                if let Some(object) = object {
                    self.index.mutations.entry(object).or_default().push(call.span.start);
                }
            }
            if member.property.name == "push" {
                if let (Some(object), Some(native)) = (object, self.native_ctx.last().copied()) {
                    self.index.pushes.push((call.span.start, object, native));
                }
            }
        }
        let mut args = CallArgs { end: call.span.end, ..CallArgs::default() };
        for argument in &call.arguments {
            let Some(expression) = argument.as_expression() else {
                continue;
            };
            // `x[N]`, or a local `a` the same statement list set to `x[N]`.
            if let Some(index) =
                literal_index(expression).or_else(|| self.aliased_index(expression, call.span.start))
            {
                args.indices.push(index);
            }
            let mut sorted =
                SortedLists { scoping: self.scoping, sorted_aliases: &self.sorted_aliases, out: Vec::new() };
            sorted.visit_expression(expression);
            args.sorted.extend(sorted.out);
        }
        if !args.indices.is_empty() || !args.sorted.is_empty() {
            self.index.call_args.insert(call.span.start, args);
        }
        walk::walk_call_expression(self, call);
    }
}

/// Collects the lists sorted (directly or through a `s = L.sort()` alias) inside an expression.
pub(crate) struct SortedLists<'s> {
    pub(crate) scoping: &'s Scoping,
    /// `s = L.sort()` bindings: s -> L.
    pub(crate) sorted_aliases: &'s HashMap<SymbolId, SymbolId>,
    pub(crate) out: Vec<SymbolId>,
}

impl<'a> Visit<'a> for SortedLists<'_> {
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if let Expression::StaticMemberExpression(member) = call.callee.without_parentheses() {
            if member.property.name == "sort" {
                if let Some(symbol) = ident_symbol(&member.object, self.scoping) {
                    self.out.push(symbol);
                }
            }
        }
        walk::walk_call_expression(self, call);
    }

    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        if let Some(list) = crate::analysis::identifier_symbol(identifier, self.scoping)
            .and_then(|symbol| self.sorted_aliases.get(&symbol))
        {
            self.out.push(*list);
        }
    }

    fn visit_arrow_function_expression(&mut self, _: &ArrowFunctionExpression<'a>) {}

    fn visit_function(&mut self, _: &Function<'a>, _: ScopeFlags) {}
}

impl ProgramIndex {
    /// Index a whole program.
    pub(crate) fn build(program: &Program<'_>, scoping: &Scoping) -> Self {
        let mut indexer = Indexer {
            scoping,
            index: ProgramIndex::default(),
            func_stack: Vec::new(),
            arg_fn_callee: HashMap::new(),
            native_ctx: Vec::new(),
            paths: HashMap::new(),
            for_in_keys: std::collections::HashSet::new(),
            method_pairs: HashMap::new(),
            sorted_aliases: HashMap::new(),
            test_vars: HashMap::new(),
            if_arms: Vec::new(),
            stmt_lists: Vec::new(),
            next_list: 0,
            index_aliases: HashMap::new(),
            plain_targets: std::collections::HashSet::new(),
        };
        indexer.visit_program(program);
        let mut index = indexer.index;
        index.facts.sort_by_key(|(offset, _)| *offset);
        index.refs.sort_by_key(|(offset, _, _)| *offset);
        index.units.sort_by_key(|((start, end), _)| (*start, u32::MAX - *end));
        index.heads.sort_by_key(|head| head.stmt.0);
        index.local_calls.sort_by_key(|(offset, _)| *offset);
        index
    }

    /// The facts inside `range`.
    pub(crate) fn facts_in(&self, range: Range) -> impl Iterator<Item = &Fact> {
        let first = self.facts.partition_point(|(offset, _)| *offset < range.0);
        self.facts[first..]
            .iter()
            .take_while(move |(offset, _)| *offset < range.1)
            .map(|(_, fact)| fact)
    }

    /// The binding references inside `range`.
    pub(crate) fn refs_in(&self, range: Range) -> impl Iterator<Item = &(u32, SymbolId, usize)> {
        let first = self.refs.partition_point(|(offset, _, _)| *offset < range.0);
        self.refs[first..].iter().take_while(move |(offset, _, _)| *offset < range.1)
    }

    /// The innermost unit containing `offset`: units nest or are disjoint,
    /// so it is the containing unit with the greatest start.
    pub(crate) fn unit_at(&self, offset: u32) -> Option<Range> {
        let end = self.units.partition_point(|((start, _), _)| *start <= offset);
        self.units[..end]
            .iter()
            .rev()
            .map(|(unit, _)| *unit)
            .find(|unit| inside(offset, *unit))
    }

    /// The innermost function containing `offset`.
    pub(crate) fn func_at(&self, offset: u32) -> usize {
        // Functions are recorded in pre-order: the innermost containing one
        // is the last containing one with a start before `offset`.
        let end = self.functions.partition_point(|function| function.0 <= offset);
        (0..end).rev().find(|id| inside(offset, self.functions[*id])).unwrap_or(usize::MAX)
    }

    /// Control heads governing `offset` inside function `func`.
    pub(crate) fn heads_governing(&self, offset: u32, func: usize) -> Vec<Range> {
        let floor = self.functions.get(func).map_or(0, |function| function.0);
        let end = self.heads.partition_point(|head| head.stmt.0 <= offset);
        self.heads[..end]
            .iter()
            .rev()
            .take_while(|head| head.stmt.0 >= floor)
            .filter(|head| head.func == func && inside(offset, head.stmt) && !inside(offset, head.head))
            .map(|head| head.head)
            .collect()
    }

    /// The statement units inside `block`.
    pub(crate) fn units_within(&self, block: Range) -> impl Iterator<Item = Range> + '_ {
        let first = self.units.partition_point(|((start, _), _)| *start < block.0);
        self.units[first..]
            .iter()
            .take_while(move |((start, _), _)| *start < block.1)
            .filter(move |(unit, _)| contains(block, *unit))
            .map(|(unit, _)| *unit)
    }

    /// The function a symbol resolves to (declaration or assigned function).
    pub(crate) fn resolve_function(&self, symbol: SymbolId, depth: usize) -> Vec<Range> {
        let mut out = Vec::new();
        if let Some(function) = self.fn_decl.get(&symbol) {
            out.push(*function);
        }
        if depth == 0 {
            return out;
        }
        for write in self.writes.get(&symbol).into_iter().flatten() {
            match write.rhs {
                Rhs::Function(function) => out.push(function),
                Rhs::Ident(next) if next != symbol => out.extend(self.resolve_function(next, depth - 1)),
                _ => {}
            }
        }
        out
    }

    /// `function` itself (not a nested function) runs a `for…in` loop.
    pub(crate) fn own_for_in(&self, function: Range) -> bool {
        let first = self.facts.partition_point(|(offset, _)| *offset < function.0);
        self.facts[first..]
            .iter()
            .take_while(|(offset, _)| *offset < function.1)
            .any(|(offset, fact)| {
                matches!(fact, Fact::ForIn) && self.functions.get(self.func_at(*offset)) == Some(&function)
            })
    }

    /// `function` creates an `iframe` element.
    pub(crate) fn builds_iframe(&self, function: Range) -> bool {
        self.facts_in(function).any(|fact| {
            matches!(fact, Fact::Call { args, .. } if args.iter().any(|arg| arg.eq_ignore_ascii_case("iframe")))
        })
    }
}

/// The slot-local facts of one slot.
#[derive(Debug, Default)]
pub(crate) struct Census {
    pub(crate) members: BTreeSet<String>,
    pub(crate) calls: Vec<(String, Vec<String>, bool)>,
    pub(crate) globals: BTreeSet<String>,
    pub(crate) strings: BTreeSet<String>,
    pub(crate) regexes: BTreeSet<String>,
    pub(crate) for_in: bool,
    /// A table is indexed by a `for…in` key ([`Fact::KeyLookup`]).
    pub(crate) key_lookup: bool,
    /// Some value is read through a callback parameter of an iframe builder.
    pub(crate) frame_realm: bool,
    pub(crate) path_tables: Vec<Vec<Vec<String>>>,
    /// Per own emission site, its value writes (outermost anchored calls
    /// before it, in source order): numeric index arguments and sorted list
    /// identities.
    pub(crate) write_groups: Vec<Vec<CallArgs>>,
    /// `L.push` under a native-code test: list -> native.
    pub(crate) list_native: HashMap<SymbolId, BTreeSet<bool>>,
    pub(crate) units: usize,
    /// Member and string names of the statements that write this slot's
    /// emitted buffers directly (`buf[4] = (flags) ^ mask()`).
    pub(crate) direct: BTreeSet<String>,
}

impl Census {
    /// A member `name` is read.
    pub(crate) fn member(&self, name: &str) -> bool {
        self.members.contains(name)
    }

    /// `method` is called.
    pub(crate) fn calls(&self, method: &str) -> bool {
        self.calls.iter().any(|(m, _, _)| m == method)
    }

    /// `method` is called with the string argument `arg`.
    pub(crate) fn calls_with(&self, method: &str, arg: &str) -> bool {
        self.calls.iter().any(|(m, args, _)| m == method && args.iter().any(|a| a == arg))
    }

    /// The string literal `value` occurs.
    pub(crate) fn string(&self, value: &str) -> bool {
        self.strings.contains(value)
    }

    /// Some string literal contains `needle`.
    pub(crate) fn string_containing(&self, needle: &str) -> bool {
        self.strings.iter().any(|s| s.contains(needle)) || self.regexes.iter().any(|s| s.contains(needle))
    }

    /// Member read or string literal (`x.name`, `"name" in x`,
    /// `hasOwnProperty.call(x, "name")`).
    pub(crate) fn names(&self, name: &str) -> bool {
        self.member(name) || self.string(name)
    }
}

/// Build the slot-local census of `view` from the program index.
pub(crate) fn build_census(index: &ProgramIndex, view: &SlotView, scoping: &Scoping) -> Census {
    let spans: Vec<Range> = view.spans().iter().map(|span| (span.start, span.end)).collect();
    let in_producer = |offset: u32| spans.iter().any(|span| inside(offset, *span));
    let anchors = view.anchors();
    let own: Vec<(u32, SymbolId)> = index
        .emissions
        .iter()
        .filter(|(offset, _)| anchors.binary_search(offset).is_ok())
        .copied()
        .collect();
    let observers: BTreeSet<SymbolId> = own.iter().map(|(_, symbol)| *symbol).collect();
    // 1. blocks
    let mut blocks: Vec<Range> = Vec::new();
    for (site, _) in &own {
        let mut best: Option<Range> = None;
        let mut candidates: Vec<Range> = index
            .stmts
            .iter()
            .filter(|stmt| inside(*site, **stmt) && spans.iter().any(|span| contains(*span, **stmt)))
            .copied()
            .collect();
        candidates.sort_by_key(|stmt| stmt.1 - stmt.0);
        for stmt in candidates {
            let exclusive = index
                .emissions
                .iter()
                .filter(|(offset, _)| inside(*offset, stmt))
                .all(|(_, observer)| observers.contains(observer));
            if !exclusive {
                break;
            }
            best = Some(stmt);
        }
        if let Some(block) = best {
            blocks.push(block);
        }
    }
    // 2. units
    let mut units: BTreeSet<Range> = BTreeSet::new();
    for block in &blocks {
        units.extend(index.units_within(*block));
    }
    for anchor in anchors {
        if in_producer(*anchor) {
            if let Some(unit) = index.unit_at(*anchor) {
                units.insert(unit);
            }
        }
    }
    // 3. dataflow
    let mut census = Census::default();
    let mut seen_symbols: BTreeSet<SymbolId> = BTreeSet::new();
    let mut frontier: Vec<Range> = units.iter().copied().collect();
    let mut rounds = 0;
    while !frontier.is_empty() && rounds < 8 && units.len() < 600 {
        rounds += 1;
        let mut next = Vec::new();
        for unit in frontier {
            for (offset, symbol, func) in index.refs_in(unit) {
                if index.write_targets.contains(offset) {
                    continue;
                }
                let symbol = *symbol;
                // The emitter is where the values go, not one of them: a
                // register that held it (`e = r; e.next(…)`) and later a
                // neighbouring probe's value (`r = typeof X …`) carries
                // nothing of this slot.
                if observers.contains(&symbol) {
                    continue;
                }
                if let Some(callee) = index.param_callee.get(&symbol) {
                    if index.resolve_function(*callee, 3).iter().any(|f| index.builds_iframe(*f)) {
                        census.frame_realm = true;
                    }
                }
                let declared = scoping.symbol_span(symbol).start;
                if !in_producer(declared) {
                    continue;
                }
                let Some(writes) = index.writes.get(&symbol) else {
                    continue;
                };
                // Reaching write: the latest earlier write in the reading
                // function or, failing that, in the nearest enclosing
                // function; a value only written by sibling closures (a
                // flag set by a callback) takes every write.
                let mut same: Option<&Write> = None;
                let mut scope = *func;
                while scope != usize::MAX {
                    same = writes
                        .iter()
                        .filter(|write| write.func == scope && write.offset < unit.0)
                        .max_by_key(|write| write.offset);
                    if same.is_some() {
                        break;
                    }
                    scope = index.parents.get(scope).copied().unwrap_or(usize::MAX);
                }
                let picked: Vec<&Write> = match same {
                    Some(write) => vec![write],
                    None => writes.iter().collect(),
                };
                let _ = offset;
                if !seen_symbols.insert(symbol) && same.is_none() {
                    continue;
                }
                for write in picked {
                    if !in_producer(write.offset) {
                        continue;
                    }
                    let mut add = Vec::new();
                    if let Some(found) = index.unit_at(write.offset) {
                        add.push(found);
                    }
                    add.extend(index.heads_governing(write.offset, write.func));
                    // Mutations between this write and the next write of the
                    // same function, inside that function.
                    let scope_range = index.functions.get(write.func).copied().unwrap_or((0, u32::MAX));
                    let next_write = writes
                        .iter()
                        .filter(|other| other.func == write.func && other.offset > write.offset)
                        .map(|other| other.offset)
                        .min()
                        .unwrap_or(scope_range.1);
                    for mutation in index.mutations.get(&symbol).into_iter().flatten() {
                        if *mutation > write.offset && *mutation < next_write && inside(*mutation, scope_range) {
                            if let Some(found) = index.unit_at(*mutation) {
                                add.push(found);
                            }
                            add.extend(
                                index
                                    .heads_governing(*mutation, index.func_at(*mutation))
                                    .into_iter(),
                            );
                        }
                    }
                    for found in add {
                        if units.insert(found) {
                            next.push(found);
                        }
                    }
                }
            }
        }
        frontier = next;
    }
    // 4. facts
    let mut regions: Vec<Range> = blocks.clone();
    regions.extend(units.iter().copied());
    regions.sort_unstable();
    let mut merged: Vec<Range> = Vec::new();
    for region in regions {
        match merged.last_mut() {
            Some(last) if region.0 < last.1 => last.1 = last.1.max(region.1),
            _ => merged.push(region),
        }
    }
    for region in &merged {
        for fact in index.facts_in(*region) {
            match fact {
                Fact::Member(name) => {
                    census.members.insert(name.clone());
                }
                Fact::Call { method, args, construct } => census.calls.push((method.clone(), args.clone(), *construct)),
                Fact::Global(name) => {
                    census.globals.insert(name.clone());
                }
                Fact::Str(value) => {
                    census.strings.insert(value.clone());
                }
                Fact::Regex(value) => {
                    census.regexes.insert(value.clone());
                }
                Fact::ForIn => census.for_in = true,
                Fact::KeyLookup => census.key_lookup = true,
                Fact::PathTable(table) => census.path_tables.push(table.clone()),
            }
        }
        for (offset, list, native) in &index.pushes {
            if inside(*offset, *region) {
                census.list_native.entry(*list).or_default().insert(*native);
            }
        }
        // A key-collecting helper (`function keys(o) { var k = []; for (var
        // x in o) k[k.length] = x; return k; }`) called here is this slot's
        // `for…in` walk.
        if !census.for_in {
            let first = index.local_calls.partition_point(|(offset, _)| *offset < region.0);
            census.for_in = index.local_calls[first..]
                .iter()
                .take_while(|(offset, _)| *offset < region.1)
                .any(|(_, callee)| {
                    index.resolve_function(*callee, 3).iter().any(|function| index.own_for_in(*function))
                });
        }
    }
    // Own value writes: the anchored calls in evaluation order, grouped by
    // the emission site that follows them. A write nested in another
    // write's arguments (`w(k, x[0], b, w(k, x[3], b, p))`) runs first, so
    // the order is by call end (post-order), not by call start.
    // Sorted-list identities are read through nested calls already, so a
    // nested write contributes its own index arguments only.
    let mut covered = 0;
    let mut group: Vec<CallArgs> = Vec::new();
    for anchor in anchors {
        if own.iter().any(|(site, _)| site == anchor) {
            group.sort_by_key(|args| args.end);
            census.write_groups.push(std::mem::take(&mut group));
            continue;
        }
        if let Some(args) = index.call_args.get(anchor) {
            if *anchor < covered {
                if !args.indices.is_empty() {
                    group.push(CallArgs { sorted: Vec::new(), ..args.clone() });
                }
                continue;
            }
            covered = args.end;
            group.push(args.clone());
        }
    }
    census.units = units.len();
    // 5. direct writes: statements of the emitting function that mention the
    // emitted buffer before the emission.
    for (site, _) in &own {
        let Some(buffer) = index.emission_buffer.get(site) else {
            continue;
        };
        let function = index.functions.get(index.func_at(*site)).copied().unwrap_or((0, *site));
        let mut direct_units: BTreeSet<Range> = BTreeSet::new();
        for (offset, symbol, _) in index.refs_in((function.0, *site)) {
            if symbol == buffer {
                if let Some(unit) = index.unit_at(*offset) {
                    direct_units.insert(unit);
                }
            }
        }
        for unit in direct_units {
            for fact in index.facts_in(unit) {
                match fact {
                    Fact::Member(name) | Fact::Str(name) => {
                        census.direct.insert(name.clone());
                    }
                    _ => {}
                }
            }
        }
    }
    census
}

/// Memo key of the program index (per analysis, in [`FinderCtx::memo`]).
#[derive(Hash, PartialEq, Eq)]
pub(crate) struct ProgramIndexKey;

/// Memo key of one slot's census.
#[derive(Hash, PartialEq, Eq)]
pub(crate) struct CensusKey(pub(crate) Vec<u32>, pub(crate) u32);

/// The census memo key of a slot (anchors and producer start).
pub(crate) fn slot_key(view: &SlotView) -> CensusKey {
    CensusKey(view.anchors().to_vec(), view.producer_span().map_or(0, |span| span.start))
}

/// The slot-local census of `view` (memoized per analysis and slot; the
/// binders read the census their recognizer built).
pub(crate) fn census(view: &SlotView, ctx: &FinderCtx) -> Rc<Census> {
    ctx.memo(slot_key(view), || {
        let index = ctx.memo(ProgramIndexKey, || ProgramIndex::build(ctx.parse.program, ctx.parse.scoping));
        build_census(&index, view, ctx.parse.scoping)
    })
}

#[cfg(test)]
mod tests {
    fn index_of(source: &str) -> super::ProgramIndex {
        let allocator = oxc_allocator::Allocator::default();
        let parsed = oxc_parser::Parser::new(&allocator, source, oxc_span::SourceType::mjs()).parse();
        assert!(!parsed.panicked);
        let semantic = oxc_semantic::SemanticBuilder::new().build(&parsed.program).semantic;
        let (scoping, _) = semantic.into_scoping_and_nodes();
        super::ProgramIndex::build(&parsed.program, &scoping)
    }

    /// Lists pushed under a native-code test get the test's polarity, for
    /// negated, conjoined and plain tests.
    #[test]
    fn native_test_polarity_of_pushes() {
        let index = index_of(
            "let a = [], b = [];
             q((n) => {
               if (typeof f === 'function' && !/native code/.test(String(f))) { a.push(n); } else { b.push(n); }
               if (re.test(String(g))) { b.push(n); } else { a.push(n); }
             });",
        );
        let polarity: Vec<bool> = index.pushes.iter().map(|(_, _, native)| *native).collect();
        assert_eq!(polarity, [false, true, true, false]);
        let lists: Vec<_> = index.pushes.iter().map(|(_, list, _)| *list).collect();
        assert_eq!(lists[0], lists[3]);
        assert_eq!(lists[1], lists[2]);
        assert_ne!(lists[0], lists[1]);
    }

    /// A callback parameter of a helper that builds an iframe is a frame
    /// realm; the same callback shape under a plain scheduler is not.
    #[test]
    fn frame_realm_through_iframe_builder() {
        let index = index_of(
            "function build(cb) { let f = make(document, 'iframe'); cb(f.contentWindow); }
             let viaFrame = build;
             function later(cb) { setTimeout(cb, 0); }
             viaFrame((w) => { w.document.createElement('a'); });
             later((w) => { w.document.createElement('a'); });",
        );
        let realms: Vec<bool> = index
            .param_callee
            .values()
            .map(|callee| index.resolve_function(*callee, 3).iter().any(|function| index.builds_iframe(*function)))
            .collect();
        let mut sorted = realms.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, [false, true]);
    }

    /// Path tables are arrays of string-literal paths; generated member and
    /// callee spellings are not facts, but a generated callee keeps its
    /// string arguments.
    #[test]
    fn path_tables_and_generated_names() {
        let index = index_of(
            "let t = [['Navigator', 'prototype'], ['safari']];
             $AB.$CD;
             $AB('iframe');",
        );
        let tables: Vec<_> = index
            .facts
            .iter()
            .filter_map(|(_, fact)| match fact {
                super::Fact::PathTable(table) => Some(table.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(tables, [vec![vec!["Navigator".to_string(), "prototype".to_string()], vec!["safari".to_string()]]]);
        assert!(!index.facts.iter().any(|(_, fact)| matches!(fact, super::Fact::Member(name) if name.starts_with('$'))));
        assert!(index
            .facts
            .iter()
            .any(|(_, fact)| matches!(fact, super::Fact::Call { method, args, .. } if method.is_empty() && args == &["iframe"])));
    }

    /// A local set to `x[N]` by a statement of the same list stands for
    /// `x[N]` in later call arguments, including inside its own
    /// reassignment's value; not after that write, not under a branch, and
    /// not in another statement list.
    #[test]
    fn tuple_index_through_a_local() {
        let source = "function f(x, k, b) {
               let a = x[0];
               let s = w(k, x[2], b, 3);
               a = w(k, x[1], b, w(k, a, b, s));
               w(k, a, b, 9);
               let c = x[3];
               if (k) { c = x[4]; }
               w(k, c, b, 1);
               let d = x[5];
               g(() => w(k, d, b, 2));
               if (k) d = 1;
               w(k, d, b, 3);
             }";
        let index = index_of(source);
        let mut calls: Vec<_> = index.call_args.iter().map(|(start, args)| (*start, args.indices.clone())).collect();
        calls.sort();
        let indices: Vec<Vec<u32>> = calls.into_iter().map(|(_, indices)| indices).collect();
        assert_eq!(indices, [vec![2], vec![1], vec![0]]);
    }
}
