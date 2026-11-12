//! Expand small tuple helpers at `let [a, b] = H(x, y);` call sites, and at
//! `let t = H(x, y);` when `t` is only ever read as `t[0]`, `t[1]`, ….
//!
//! A helper qualifies when it is a never-reassigned `$VMHELPER_`/`vm_sub_`
//! function declaration with plain parameters it never writes, whose body is
//! at most a few `let`/`const` declarations and expression statements ending
//! in `return [e0, e1, …];` (no spread or hole), and which mentions no
//! `this`, `arguments`, nested function or class, `await`, `yield`, or
//! comment. Every other name it reads must resolve to the same binding at
//! the call site.
//!
//! The call statement becomes, in order: one fresh `$SSA_` temporary per
//! non-stable argument (arguments still run first, left to right), the body
//! with parameters replaced and locals renamed fresh, then `let s_k = e_k;`
//! per named slot (an unnamed slot with an effect stays as `(e_k);`); a
//! held tuple gets a fresh name per index it reads, and each `t[k]` read
//! becomes that name. A
//! stable argument — a literal or a never-written local that is not a slot —
//! is substituted directly, since reading it later sees the same value. The
//! helper itself goes once every reference has been expanded. The forwarding
//! pass that follows removes the copies this leaves.

use std::collections::{BTreeMap, BTreeSet};

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast::AstKind;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, ScopeId, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::operator::UnaryOperator;
use oxc_syntax::scope::ScopeFlags;

use super::super::purity::expression_is_pure;
use crate::analysis;
use super::super::support::whole_line;
use crate::transforms::emitted_js::support::indent_at;

/// Helpers longer than this are left as calls.
const MAX_BODY_STATEMENTS: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InlinedTupleHelpers {
    pub code: String,
    pub expanded: usize,
}

pub(crate) fn inline_tuple_helpers(source: &str) -> Result<InlinedTupleHelpers, String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let comments: Vec<Span> = program.comments.iter().map(|comment| comment.span).collect();
    let mut helpers = Helpers {
        source,
        scoping: &scoping,
        comments: &comments,
        found: BTreeMap::new(),
        dynamic: false,
    };
    helpers.visit_program(&program);
    if helpers.dynamic || helpers.found.is_empty() {
        return Ok(unchanged(source));
    }
    let found = helpers.found;
    let mut calls = Calls {
        source,
        scoping: &scoping,
        nodes: &nodes,
        helpers: &found,
        comments: &comments,
        names: FreshNames::new(source),
        expansions: Vec::new(),
    };
    super::super::support::for_each_list(&program, |list| calls.list(list));
    // A held tuple read inside another expanded call statement would be
    // copied as text there: leave that holder as a call.
    let statements: Vec<Span> = calls.expansions.iter().map(|e| e.statement).collect();
    let mut edits = Vec::new();
    let mut counts: BTreeMap<SymbolId, usize> = BTreeMap::new();
    for expansion in calls.expansions {
        let nested = expansion.reads.iter().any(|(read, _)| {
            statements
                .iter()
                .any(|outer| *outer != expansion.statement && outer.start <= read.start && read.end <= outer.end)
        });
        if nested {
            continue;
        }
        edits.push(analysis::Edit::replace(expansion.statement, expansion.text));
        for (read, name) in expansion.reads {
            edits.push(analysis::Edit::replace(read, name));
        }
        *counts.entry(expansion.helper).or_default() += 1;
    }
    if edits.is_empty() {
        return Ok(unchanged(source));
    }
    let mut expanded = 0;
    for (helper, count) in &counts {
        expanded += count;
        let references = scoping.get_resolved_reference_ids(*helper).len();
        if references == *count {
            edits.push(analysis::Edit::delete(whole_line(source, found[helper].declaration)));
        }
    }
    let applied = analysis::apply_edits(source, edits);
    if !applied.rejected.is_empty() {
        return Err("tuple helper expansion edits overlap".into());
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output)
        .map_err(|error| format!("tuple helper expansion output: {error}"))?;
    Ok(InlinedTupleHelpers {
        code: applied.output,
        expanded,
    })
}

fn unchanged(source: &str) -> InlinedTupleHelpers {
    InlinedTupleHelpers {
        code: source.into(),
        expanded: 0,
    }
}

/// A helper body as text pieces with its parameter and local reads marked.
struct Helper {
    declaration: Span,
    parameters: Vec<SymbolId>,
    /// Body statements before the return, as source spans.
    statements: Vec<Span>,
    elements: Vec<Span>,
    /// Elements that are comma expressions: they need parentheses.
    sequence_elements: Vec<bool>,
    /// Elements that are pure to evaluate and drop.
    pure_elements: Vec<bool>,
    /// Locals declared in the body.
    locals: Vec<SymbolId>,
    /// Every identifier inside the body: its span and what it names.
    names: Vec<(Span, Name)>,
    /// Names read from outside: they must resolve the same at a call site.
    free: Vec<(String, Option<SymbolId>)>,
}

#[derive(Clone, Copy)]
enum Name {
    Parameter(usize),
    Local(usize),
}

struct Helpers<'s> {
    source: &'s str,
    scoping: &'s Scoping,
    comments: &'s [Span],
    found: BTreeMap<SymbolId, Helper>,
    dynamic: bool,
}

impl Helpers<'_> {
    fn helper(&self, function: &Function<'_>) -> Option<(SymbolId, Helper)> {
        let id = function.id.as_ref()?;
        let symbol = id.symbol_id.get()?;
        if !(id.name.starts_with("$VMHELPER_") || id.name.starts_with("vm_sub_"))
            || function.r#async
            || function.generator
            || function.params.rest.is_some()
            || self.scoping.symbol_is_mutated(symbol)
            || has_comment(self.comments, function.span)
        {
            return None;
        }
        let body = function.body.as_ref()?;
        if !body.directives.is_empty() || body.statements.len() > MAX_BODY_STATEMENTS + 1 {
            return None;
        }
        let (last, rest) = body.statements.split_last()?;
        let Statement::ReturnStatement(ret) = last else {
            return None;
        };
        let Some(Expression::ArrayExpression(array)) = &ret.argument else {
            return None;
        };
        let mut parameters = Vec::new();
        for parameter in &function.params.items {
            let BindingPattern::BindingIdentifier(binding) = &parameter.pattern else {
                return None;
            };
            let parameter_symbol = binding.symbol_id.get()?;
            if self.scoping.symbol_is_mutated(parameter_symbol) {
                return None;
            }
            parameters.push(parameter_symbol);
        }
        let mut locals = Vec::new();
        for statement in rest {
            match statement {
                Statement::VariableDeclaration(declaration)
                    if declaration.kind != VariableDeclarationKind::Var
                        && declaration.declarations.len() == 1 =>
                {
                    let BindingPattern::BindingIdentifier(binding) =
                        &declaration.declarations[0].id
                    else {
                        return None;
                    };
                    declaration.declarations[0].init.as_ref()?;
                    locals.push(binding.symbol_id.get()?);
                }
                Statement::ExpressionStatement(_) => {}
                _ => return None,
            }
        }
        let mut elements = Vec::new();
        let mut sequence_elements = Vec::new();
        let mut pure_elements = Vec::new();
        for element in &array.elements {
            let expression = element.as_expression()?;
            elements.push(expression.span());
            sequence_elements.push(matches!(expression, Expression::SequenceExpression(_)));
            // A parameter or local read is pure; a free name may throw.
            let own = matches!(expression, Expression::Identifier(id)
                if id.reference_id.get().and_then(|r| self.scoping.get_reference(r).symbol_id())
                    .is_some_and(|t| parameters.contains(&t) || locals.contains(&t)));
            pure_elements.push(own || expression_is_pure(expression));
        }
        let mut closed = Closed::default();
        closed.visit_function_body(body);
        if !closed.closed {
            return None;
        }
        let mut names = Vec::new();
        let mut free = Vec::new();
        for (span, reference) in closed.references {
            let target = self.scoping.get_reference(reference).symbol_id();
            if let Some(index) = target.and_then(|t| parameters.iter().position(|p| *p == t)) {
                names.push((span, Name::Parameter(index)));
            } else if let Some(index) = target.and_then(|t| locals.iter().position(|l| *l == t)) {
                names.push((span, Name::Local(index)));
            } else {
                free.push((self.source[span.start as usize..span.end as usize].to_string(), target));
            }
        }
        for (span, local) in closed.bindings {
            if let Some(index) = locals.iter().position(|l| *l == local) {
                names.push((span, Name::Local(index)));
            }
        }
        names.sort_by_key(|(span, _)| span.start);
        Some((
            symbol,
            Helper {
                declaration: function.span,
                parameters,
                statements: rest.iter().map(GetSpan::span).collect(),
                elements,
                sequence_elements,
                pure_elements,
                locals,
                names,
                free,
            },
        ))
    }
}

impl<'a> Visit<'a> for Helpers<'_> {
    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        if let Some((symbol, helper)) = self.helper(function) {
            self.found.insert(symbol, helper);
        }
        walk::walk_function(self, function, flags);
    }

    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        if identifier.name == "eval" {
            self.dynamic = true;
        }
    }

    fn visit_with_statement(&mut self, statement: &WithStatement<'a>) {
        self.dynamic = true;
        walk::walk_with_statement(self, statement);
    }
}

/// Collects a body's identifiers and rejects what cannot move into a caller.
struct Closed {
    closed: bool,
    references: Vec<(Span, oxc_semantic::ReferenceId)>,
    bindings: Vec<(Span, SymbolId)>,
}

impl Default for Closed {
    fn default() -> Self {
        Self {
            closed: true,
            references: Vec::new(),
            bindings: Vec::new(),
        }
    }
}

impl<'a> Visit<'a> for Closed {
    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        if identifier.name == "arguments" {
            self.closed = false;
        }
        if let Some(reference) = identifier.reference_id.get() {
            self.references.push((identifier.span, reference));
        }
    }
    fn visit_binding_identifier(&mut self, binding: &BindingIdentifier<'a>) {
        if let Some(symbol) = binding.symbol_id.get() {
            self.bindings.push((binding.span, symbol));
        }
    }
    fn visit_this_expression(&mut self, _: &ThisExpression) {
        self.closed = false;
    }
    fn visit_super(&mut self, _: &Super) {
        self.closed = false;
    }
    fn visit_meta_property(&mut self, _: &MetaProperty<'a>) {
        self.closed = false;
    }
    fn visit_function(&mut self, _: &Function<'a>, _: ScopeFlags) {
        self.closed = false;
    }
    fn visit_arrow_function_expression(&mut self, _: &ArrowFunctionExpression<'a>) {
        self.closed = false;
    }
    fn visit_class(&mut self, _: &Class<'a>) {
        self.closed = false;
    }
    fn visit_await_expression(&mut self, _: &AwaitExpression<'a>) {
        self.closed = false;
    }
    fn visit_yield_expression(&mut self, _: &YieldExpression<'a>) {
        self.closed = false;
    }
}

/// Fresh `$SSA_` names that occur nowhere in the source.
/// Numbers `n` for which `{prefix}{n}` occurs somewhere in `source` as a
/// substring: every leading digit run after an occurrence of `prefix`
/// (`$SSA_t12` makes both `1` and `12` taken). One scan answers what a
/// `source.contains` per candidate name would.
pub(crate) fn taken_numbers(source: &str, prefix: &str) -> std::collections::BTreeSet<String> {
    let mut taken = std::collections::BTreeSet::new();
    for (start, _) in source.match_indices(prefix) {
        let digits = &source[start + prefix.len()..];
        let length = digits.bytes().take_while(u8::is_ascii_digit).count();
        for end in 1..=length {
            taken.insert(digits[..end].to_string());
        }
    }
    taken
}

/// Marks `{prefix}{n}` as present, as [`taken_numbers`] would see it.
pub(crate) fn take_number(taken: &mut std::collections::BTreeSet<String>, n: usize) {
    let digits = n.to_string();
    for end in 1..=digits.len() {
        taken.insert(digits[..end].to_string());
    }
}

pub(crate) struct FreshNames {
    taken: BTreeSet<String>,
    next: usize,
}

impl FreshNames {
    pub(crate) fn new(source: &str) -> Self {
        let taken = source
            .match_indices("$SSA_")
            .map(|(start, _)| {
                let tail = &source[start..];
                let end = tail
                    .bytes()
                    .position(|b| !(b.is_ascii_alphanumeric() || b == b'_' || b == b'$'))
                    .unwrap_or(tail.len());
                tail[..end].to_string()
            })
            .collect();
        Self { taken, next: 1 }
    }

    pub(crate) fn fresh(&mut self) -> String {
        loop {
            let name = format!("$SSA_x{}", self.next);
            self.next += 1;
            if self.taken.insert(name.clone()) {
                return name;
            }
        }
    }
}

struct Calls<'s, 'a> {
    source: &'s str,
    scoping: &'s Scoping,
    nodes: &'s AstNodes<'a>,
    helpers: &'s BTreeMap<SymbolId, Helper>,
    comments: &'s [Span],
    names: FreshNames,
    expansions: Vec<Expansion>,
}

/// One planned call-site expansion.
struct Expansion {
    helper: SymbolId,
    statement: Span,
    text: String,
    /// `t[k]` reads of a held tuple and the names that replace them.
    reads: Vec<(Span, String)>,
}

/// A `let`/`const` declaration whose initializer calls a known helper.
struct CallSite<'b, 'a, 's> {
    declaration: &'b VariableDeclaration<'a>,
    declarator: &'b VariableDeclarator<'a>,
    call: &'b CallExpression<'a>,
    callee: &'b IdentifierReference<'a>,
    helper_symbol: SymbolId,
    helper: &'s Helper,
}

impl<'s> Calls<'s, '_> {
    fn statement(&mut self, statement: &Statement<'_>) {
        let Some(CallSite {
            declaration,
            declarator,
            call,
            callee,
            helper_symbol,
            helper,
        }) = self.call_site(statement)
        else {
            return;
        };
        let Some((slots, reads)) = self.slots(&declarator.id, helper) else {
            return;
        };
        let Some(scope) = self.scope_at(callee.reference_id.get()) else {
            return;
        };
        if helper.free.iter().any(|(name, symbol)| {
            self.scoping.find_binding(scope, name.as_str().into()) != *symbol
        }) {
            return;
        }
        let mut arguments = Vec::new();
        for argument in &call.arguments {
            let Some(expression) = argument.as_expression() else {
                return;
            };
            arguments.push(expression);
        }
        let text = self.expansion_text(statement.span(), declaration.kind, helper, &arguments, &slots);
        self.expansions.push(Expansion {
            helper: helper_symbol,
            statement: statement.span(),
            text,
            reads,
        });
    }

    /// The declaration's helper call, unless it is optional, has a comment,
    /// or sits inside the helper itself.
    fn call_site<'b, 'a>(&self, statement: &'b Statement<'a>) -> Option<CallSite<'b, 'a, 's>> {
        let Statement::VariableDeclaration(declaration) = statement else {
            return None;
        };
        if declaration.kind == VariableDeclarationKind::Var || declaration.declarations.len() != 1
        {
            return None;
        }
        let declarator = &declaration.declarations[0];
        let Some(Expression::CallExpression(call)) = declarator.init.as_ref() else {
            return None;
        };
        let Expression::Identifier(callee) = &call.callee else {
            return None;
        };
        let Some(helper_symbol) = analysis::identifier_symbol(callee, self.scoping) else {
            return None;
        };
        let Some(helper) = self.helpers.get(&helper_symbol) else {
            return None;
        };
        if call.optional
            || has_comment(self.comments, statement.span())
            || helper.declaration.start <= statement.span().start
                && statement.span().end <= helper.declaration.end
        {
            return None;
        }
        Some(CallSite {
            declaration,
            declarator,
            call,
            callee,
            helper_symbol,
            helper,
        })
    }

    /// The name each tuple index binds (`None` for an unnamed slot), and for a
    /// held tuple the `t[k]` reads with the fresh names that replace them.
    fn slots(
        &mut self,
        id: &BindingPattern<'_>,
        helper: &Helper,
    ) -> Option<(Vec<Option<String>>, Vec<(Span, String)>)> {
        let mut slots: Vec<Option<String>> = Vec::new();
        let mut reads = Vec::new();
        match id {
            BindingPattern::ArrayPattern(pattern) => {
                if pattern.rest.is_some() {
                    return None;
                }
                for element in &pattern.elements {
                    match element {
                        None => slots.push(None),
                        Some(BindingPattern::BindingIdentifier(id)) => slots.push(Some(
                            self.source[id.span.start as usize..id.span.end as usize].to_string(),
                        )),
                        Some(_) => return None,
                    }
                }
            }
            BindingPattern::BindingIdentifier(holder) => {
                let Some(indexed) = self.indexed_reads(holder, helper.elements.len()) else {
                    return None;
                };
                slots = vec![None; helper.elements.len()];
                for (span, index) in indexed {
                    if slots[index].is_none() {
                        slots[index] = Some(self.names.fresh());
                    }
                    reads.push((span, slots[index].clone().expect("named above")));
                }
            }
            _ => return None,
        }
        Some((slots, reads))
    }

    /// The statements that replace the call: argument temps, the body with
    /// names substituted, then one declaration per slot.
    fn expansion_text(
        &mut self,
        statement: Span,
        declaration_kind: VariableDeclarationKind,
        helper: &Helper,
        arguments: &[&Expression<'_>],
        slots: &[Option<String>],
    ) -> String {
        let indent = indent_at(self.source, statement);
        let mut lines = Vec::new();
        let parameter_text = self.parameter_text(statement, helper, arguments, &mut lines);
        let local_text: Vec<String> = helper.locals.iter().map(|_| self.names.fresh()).collect();
        let render = |span: Span| -> String {
            let mut out = String::new();
            let mut at = span.start as usize;
            for (name_span, name) in &helper.names {
                if name_span.start < span.start || name_span.end > span.end {
                    continue;
                }
                out.push_str(&self.source[at..name_span.start as usize]);
                match name {
                    Name::Parameter(index) => out.push_str(&parameter_text[*index]),
                    Name::Local(index) => out.push_str(&local_text[*index]),
                }
                at = name_span.end as usize;
            }
            out.push_str(&self.source[at..span.end as usize]);
            out
        };
        for body_statement in &helper.statements {
            lines.push(render(*body_statement));
        }
        let kind = if declaration_kind == VariableDeclarationKind::Const {
            "const"
        } else {
            "let"
        };
        for (index, element) in helper.elements.iter().enumerate() {
            let mut value = render(*element);
            if helper.sequence_elements[index] {
                value = format!("({value})");
            }
            match slots.get(index).cloned().flatten() {
                Some(slot) => lines.push(format!("{kind} {slot} = {value};")),
                None if helper.pure_elements[index] => {}
                None => lines.push(format!("({value});")),
            }
        }
        // A slot past the tuple reads undefined.
        for slot in slots.iter().skip(helper.elements.len()).flatten() {
            lines.push(format!("{kind} {slot} = void 0;"));
        }
        lines.join(&format!("\n{indent}"))
    }

    /// The text each parameter reads as, pushing a temp to `lines` for each
    /// non-stable argument and an effect line for each effectful extra one.
    fn parameter_text(
        &mut self,
        statement: Span,
        helper: &Helper,
        arguments: &[&Expression<'_>],
        lines: &mut Vec<String>,
    ) -> Vec<String> {
        // Arguments run first, in order: a non-stable one is held in a temp.
        let mut parameter_text = Vec::new();
        for index in 0..helper.parameters.len() {
            match arguments.get(index) {
                None => parameter_text.push("(void 0)".to_string()),
                Some(argument) => {
                    let text = &self.source[argument.span().start as usize..argument.span().end as usize];
                    match self.stable_text(argument, text, statement) {
                        Some(stable) => parameter_text.push(stable),
                        None => {
                            let name = self.names.fresh();
                            let text = parenthesized(argument, text);
                            lines.push(format!("let {name} = {text};"));
                            parameter_text.push(name);
                        }
                    }
                }
            }
        }
        // Extra arguments are evaluated and dropped.
        for argument in arguments.iter().skip(helper.parameters.len()) {
            if !expression_is_pure(argument) {
                let text = &self.source[argument.span().start as usize..argument.span().end as usize];
                lines.push(format!("({text});"));

            }
        }
        parameter_text
    }

    /// Every reference of `holder` as a plain `holder[k]` read with an
    /// in-range literal `k`: the member spans and indexes.
    fn indexed_reads(&self, holder: &BindingIdentifier<'_>, length: usize) -> Option<Vec<(Span, usize)>> {
        let symbol = holder.symbol_id.get()?;
        if !(holder.name.starts_with("$SSA_") || holder.name.starts_with("$CARRY_"))
            || self.scoping.symbol_is_mutated(symbol)
        {
            return None;
        }
        let mut reads = Vec::new();
        for reference in self.scoping.get_resolved_references(symbol) {
            let node = reference.node_id();
            let span = self.nodes.kind(node).span();
            let AstKind::ComputedMemberExpression(member) = self.nodes.parent_kind(node) else {
                return None;
            };
            let Expression::NumericLiteral(index) = &member.expression else {
                return None;
            };
            let written = match self.nodes.parent_kind(self.nodes.parent_id(node)) {
                AstKind::AssignmentExpression(assignment) => assignment.left.span() == member.span,
                AstKind::UpdateExpression(_) => true,
                AstKind::UnaryExpression(unary) => unary.operator == UnaryOperator::Delete,
                _ => false,
            };
            if member.object.span() != span
                || member.optional
                || written
                || index.value.fract() != 0.0
                || index.value < 0.0
                || index.value >= length as f64
            {
                return None;
            }
            reads.push((member.span, index.value as usize));
        }
        (!reads.is_empty()).then_some(reads)
    }

    /// A literal or a never-written local that is not a VM slot or carry,
    /// already declared: reading it in the body sees the argument's value.
    fn stable_text(&self, argument: &Expression<'_>, text: &str, call: Span) -> Option<String> {
        match argument {
            Expression::StringLiteral(_) | Expression::BooleanLiteral(_) | Expression::NullLiteral(_) => {
                Some(text.to_string())
            }
            Expression::NumericLiteral(_) => Some(format!("({text})")),
            Expression::UnaryExpression(unary)
                if matches!(unary.operator, UnaryOperator::UnaryNegation | UnaryOperator::Void)
                    && matches!(unary.argument, Expression::NumericLiteral(_)) =>
            {
                Some(format!("({text})"))
            }
            Expression::Identifier(identifier) => {
                let symbol = analysis::identifier_symbol(identifier, self.scoping)?;
                let declaration = self.scoping.symbol_declaration(symbol);
                let declared = self.nodes.kind(declaration).span();
                let parameter = matches!(
                    self.nodes.kind(declaration),
                    AstKind::FormalParameter(_) | AstKind::FormalParameters(_)
                );
                let name = identifier.name.as_str();
                (!name.starts_with("$KEY_")
                    && !name.starts_with("$CARRY_")
                    && !parameter
                    && !self.scoping.symbol_is_mutated(symbol)
                    && self.scoping.symbol_declarations(symbol).count() == 1
                    && (declared.end <= call.start
                        || matches!(self.nodes.kind(declaration), AstKind::Function(_))))
                .then(|| text.to_string())
            }
            _ => None,
        }
    }

    fn scope_at(&self, reference: Option<oxc_semantic::ReferenceId>) -> Option<ScopeId> {
        let node = self.scoping.get_reference(reference?).node_id();
        Some(self.nodes.get_node(node).scope_id())
    }

    fn list(&mut self, statements: &[Statement<'_>]) {
        for statement in statements {
            self.statement(statement);
        }
    }
}


/// A comma expression cannot be a declarator's initializer bare.
fn parenthesized(expression: &Expression<'_>, text: &str) -> String {
    if matches!(expression, Expression::SequenceExpression(_)) {
        format!("({text})")
    } else {
        text.to_string()
    }
}

fn has_comment(comments: &[Span], span: Span) -> bool {
    let at = comments.partition_point(|comment| comment.start < span.start);
    comments.get(at).is_some_and(|comment| comment.start < span.end)
}

#[cfg(test)]
mod tests {
    use super::inline_tuple_helpers;

    #[test]
    fn expands_a_small_helper_at_each_call() {
        let source = "function $VMHELPER_1(a, b) {\n  let $SSA_9 = a + 1;\n  return [$SSA_9, b, a[\"next\"]];\n}\nfunction f(x, $SSA_1) {\n  let [$SSA_2, $SSA_3, $SSA_4] = $VMHELPER_1(x.p, 5);\n  let [$SSA_5, , $SSA_6] = $VMHELPER_1($SSA_1, 6);\n  return [$SSA_2, $SSA_3, $SSA_4, $SSA_5, $SSA_6];\n}";
        let result = inline_tuple_helpers(source).unwrap();
        assert_eq!(result.expanded, 2, "{}", result.code);
        assert!(!result.code.contains("function $VMHELPER_1"), "{}", result.code);
        assert!(result.code.contains("let $SSA_x1 = x.p;"), "{}", result.code);
        assert!(result.code.contains("let $SSA_x2 = $SSA_x1 + 1;"), "{}", result.code);
        assert!(result.code.contains("let $SSA_3 = (5);"), "{}", result.code);
        assert!(result.code.contains("let $SSA_4 = $SSA_x1[\"next\"];"), "{}", result.code);
        // a caller parameter could change through `arguments`: it is held
        assert!(result.code.contains("let $SSA_x3 = $SSA_1;"), "{}", result.code);
    }

    #[test]
    fn keeps_comma_arguments_and_elements_parenthesized() {
        let source = "function $VMHELPER_1(a) {\n  return [(0, a.m), a];\n}\nfunction f(o) {\n  let [$SSA_1, $SSA_2] = $VMHELPER_1((0, o[\"p\"]));\n  return [$SSA_1, $SSA_2];\n}";
        let result = inline_tuple_helpers(source).unwrap();
        assert!(result.code.contains("let $SSA_x1 = (0, o[\"p\"]);"), "{}", result.code);
        assert!(result.code.contains("let $SSA_1 = (0, $SSA_x1.m);"), "{}", result.code);
    }

    #[test]
    fn expands_a_held_tuple_read_by_index() {
        let source = "function $VMHELPER_1(a) {\n  let $SSA_9 = a.m;\n  return [$SSA_9, a.n];\n}\nfunction g() {}\nfunction f(o) {\n  let $SSA_1 = $VMHELPER_1(o);\n  return g($SSA_1[1], $SSA_1[0], $SSA_1[1]);\n}";
        let result = inline_tuple_helpers(source).unwrap();
        assert_eq!(result.expanded, 1, "{}", result.code);
        assert!(result.code.contains("return g($SSA_x1, $SSA_x2, $SSA_x1);"), "{}", result.code);
        // a holder that escapes stays a call
        let source = "function $VMHELPER_1(a) {\n  return [a, a.n];\n}\nfunction g() {}\nfunction f(o) {\n  let $SSA_1 = $VMHELPER_1(o);\n  return g($SSA_1, $SSA_1[0]);\n}";
        assert_eq!(inline_tuple_helpers(source).unwrap().expanded, 0);
    }

    #[test]
    fn keeps_helpers_that_cannot_move() {
        for helper in [
            "function $VMHELPER_1(a) {\n  return [this, a];\n}",
            "function $VMHELPER_1(a) {\n  return [arguments[0], a];\n}",
            "function $VMHELPER_1(a) {\n  a = 2;\n  return [a, a];\n}",
            "function $VMHELPER_1(a) {\n  return [() => a, a];\n}",
        ] {
            let source = format!("{helper}\nfunction f(x) {{\n  let [$SSA_1, $SSA_2] = $VMHELPER_1(x);\n  return $SSA_1;\n}}");
            assert_eq!(inline_tuple_helpers(&source).unwrap().expanded, 0, "{source}");
        }
        // a free name shadowed at the call site
        let source = "let y = 1;\nfunction $VMHELPER_1(a) {\n  return [y, a];\n}\nfunction f(x) {\n  let y = 2;\n  let [$SSA_1, $SSA_2] = $VMHELPER_1(x);\n  return $SSA_1;\n}";
        assert_eq!(inline_tuple_helpers(source).unwrap().expanded, 0);
    }
}
