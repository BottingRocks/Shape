//! Write the lowered optional chains back as `?.`.
//!
//! The capture's source was compiled for old engines, which spells `x?.a.b`
//! as a null test:
//!
//! ```js
//! x == null ? void 0 : x["a"]["b"]          // → x?.["a"]["b"]
//! ```
//!
//! and a two-link chain `p?.prototype?.m` through a flag and a temporary:
//!
//! ```js
//! let z, t;
//! if (p == null) { z = true; t = p; } else { t = p["prototype"]; z = t == null; }
//! … z ? void 0 : t["m"] …                   // → p?.["prototype"]?.["m"]
//! ```
//!
//! After that `if`, `z` is exactly `t == null`, so each read of the pair is
//! `p?.["prototype"]?.["m"]` for as long as `z` keeps the flag: statements
//! up to the first one that assigns `z` on every path, outside nested
//! functions. The flag's `if` and `t` go; `z` keeps its declaration when it
//! is used again. `p` is a name or a member chain of one, read again at each
//! use: nothing in between may assign the name or a property it reads.
//! A conditional that is called or read as an object keeps its form, since
//! parenthesised optional chains bind `this` differently.

use std::collections::{HashMap, HashSet};

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::operator::{AssignmentOperator, BinaryOperator, UnaryOperator};

use crate::analysis;
use super::super::support::whole_line;

pub(crate) fn optional_chains(source: &str) -> Result<(String, usize), String> {
    let mut code = source.to_string();
    let mut total = 0;
    // A chain nested in another one's arguments goes in a later round.
    for _ in 0..4 {
        if !code.contains("void 0") {
            break;
        }
        let (next, count) = round(&code)?;
        if count == 0 {
            break;
        }
        code = next;
        total += count;
    }
    Ok((code, total))
}

fn round(source: &str) -> Result<(String, usize), String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut chained = Chained::default();
    chained.visit_program(&program);
    let mut scan = Scan {
        source,
        scoping: &scoping,
        nodes: &nodes,
        chained: chained.spans,
        claimed: Vec::new(),
        edits: Vec::new(),
        count: 0,
    };
    scan.visit_program(&program);
    if scan.count == 0 {
        return Ok((source.into(), 0));
    }
    let output = super::super::support::apply_checked(source, scan.edits, "optional chain")?;
    Ok((output, scan.count))
}

/// Expressions called, read as an object or tagged: a chain there would
/// bind `this` or short-circuit differently.
#[derive(Default)]
struct Chained {
    spans: HashSet<Span>,
}

impl<'a> Visit<'a> for Chained {
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        self.spans.insert(call.callee.get_inner_expression().span());
        walk::walk_call_expression(self, call);
    }
    fn visit_computed_member_expression(&mut self, member: &ComputedMemberExpression<'a>) {
        self.spans.insert(member.object.get_inner_expression().span());
        walk::walk_computed_member_expression(self, member);
    }
    fn visit_static_member_expression(&mut self, member: &StaticMemberExpression<'a>) {
        self.spans.insert(member.object.get_inner_expression().span());
        walk::walk_static_member_expression(self, member);
    }
    fn visit_tagged_template_expression(&mut self, tagged: &TaggedTemplateExpression<'a>) {
        self.spans.insert(tagged.tag.get_inner_expression().span());
        walk::walk_tagged_template_expression(self, tagged);
    }
}

struct Scan<'s, 'a> {
    source: &'s str,
    scoping: &'s Scoping,
    nodes: &'s AstNodes<'a>,
    chained: HashSet<Span>,
    claimed: Vec<Span>,
    edits: Vec<analysis::Edit>,
    count: usize,
}

fn text(source: &str, span: Span) -> &str {
    &source[span.start as usize..span.end as usize]
}

fn is_undefined(expression: &Expression<'_>) -> bool {
    matches!(expression.get_inner_expression(), Expression::UnaryExpression(unary) if unary.operator == UnaryOperator::Void && matches!(unary.argument, Expression::NumericLiteral(_)))
}

/// `x`, `this`, `x["a"].b[0]`: read again without effects.
fn pure_path(expression: &Expression<'_>) -> bool {
    match expression.get_inner_expression() {
        Expression::Identifier(_) | Expression::ThisExpression(_) => true,
        Expression::StaticMemberExpression(member) => !member.optional && pure_path(&member.object),
        Expression::ComputedMemberExpression(member) => {
            !member.optional
                && matches!(member.expression, Expression::StringLiteral(_) | Expression::NumericLiteral(_))
                && pure_path(&member.object)
        }
        _ => false,
    }
}

/// The innermost object of a member/call chain: `x` in `x["a"]["b"](c)`.
fn chain_base<'e, 'a>(expression: &'e Expression<'a>) -> Option<&'e Expression<'a>> {
    let mut at = expression.get_inner_expression();
    let mut links = 0;
    loop {
        at = match at {
            Expression::StaticMemberExpression(member) if !member.optional => &member.object,
            Expression::ComputedMemberExpression(member) if !member.optional => &member.object,
            Expression::CallExpression(call) if !call.optional => &call.callee,
            _ => break,
        }
        .get_inner_expression();
        links += 1;
    }
    (links > 0).then_some(at)
}

/// The link of a member/call chain that reads `path`: `x["a"]` in
/// `x["a"]["b"](c)` for the path `x["a"]`.
fn chain_link<'e, 'a>(expression: &'e Expression<'a>, source: &str, path: &str) -> Option<&'e Expression<'a>> {
    let mut at = expression.get_inner_expression();
    loop {
        at = match at {
            Expression::StaticMemberExpression(member) if !member.optional => &member.object,
            Expression::ComputedMemberExpression(member) if !member.optional => &member.object,
            Expression::CallExpression(call) if !call.optional => &call.callee,
            _ => return None,
        }
        .get_inner_expression();
        if text(source, at.span()) == path {
            return Some(at);
        }
    }
}

/// `.b` → `?.b`, `["b"]` → `?.["b"]`, `(c)` → `?.(c)`.
fn optional_link(rest: &str) -> String {
    if rest.starts_with('.') {
        format!("?{rest}")
    } else {
        format!("?.{rest}")
    }
}

/// The links after a chain's base, up to `whole`'s end, when nothing but
/// parentheses wrap the base: `["a"](c)` in `((x))["a"](c)`.
fn link_text(source: &str, base: Span, whole: Span) -> Option<String> {
    let opened = &source[whole.start as usize..base.start as usize];
    let rest = &source[base.end as usize..whole.end as usize];
    let closed = rest.len() - rest.trim_start_matches([')', ' ']).len();
    let rest = &rest[closed..];
    (opened.bytes().all(|b| b == b'(' || b == b' ')
        && opened.matches('(').count() == source[base.end as usize..base.end as usize + closed].matches(')').count()
        && rest.starts_with(['.', '[', '(']))
    .then(|| rest.to_string())
}

/// `typeof x.a !== "undefined" ? x.a : void 0` is `x.a`: the member, for a
/// member path.
fn defined_or_undefined<'e, 'a>(expression: &'e Expression<'a>, source: &str) -> Option<&'e Expression<'a>> {
    let Expression::ConditionalExpression(conditional) = expression.get_inner_expression() else {
        return None;
    };
    let Expression::BinaryExpression(binary) = conditional.test.get_inner_expression() else {
        return None;
    };
    let typed = match (binary.left.get_inner_expression(), binary.right.get_inner_expression()) {
        (Expression::UnaryExpression(unary), Expression::StringLiteral(name)) | (Expression::StringLiteral(name), Expression::UnaryExpression(unary))
            if unary.operator == UnaryOperator::Typeof && name.value == "undefined" =>
        {
            unary.argument.get_inner_expression()
        }
        _ => return None,
    };
    let member = conditional.consequent.get_inner_expression();
    (binary.operator == BinaryOperator::StrictInequality
        && is_undefined(&conditional.alternate)
        && matches!(member, Expression::StaticMemberExpression(_) | Expression::ComputedMemberExpression(_))
        && pure_path(member)
        && text(source, typed.span()) == text(source, member.span()))
    .then_some(member)
}

/// `p == null` or `null == p`: `p`.
fn null_test<'e, 'a>(test: &'e Expression<'a>) -> Option<&'e Expression<'a>> {
    let Expression::BinaryExpression(binary) = test.get_inner_expression() else {
        return None;
    };
    if binary.operator != BinaryOperator::Equality {
        return None;
    }
    match (&binary.left, &binary.right) {
        (value, Expression::NullLiteral(_)) | (Expression::NullLiteral(_), value) => Some(value.get_inner_expression()),
        _ => None,
    }
}

fn assigned_name<'e, 'a>(statement: &'e Statement<'a>) -> Option<(&'e IdentifierReference<'a>, &'e Expression<'a>)> {
    let Statement::ExpressionStatement(statement) = statement else {
        return None;
    };
    let Expression::AssignmentExpression(assignment) = &statement.expression else {
        return None;
    };
    let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else {
        return None;
    };
    (assignment.operator == AssignmentOperator::Assign).then_some((target.as_ref(), &assignment.right))
}

fn branch<'e, 'a>(statement: &'e Statement<'a>) -> &'e [Statement<'a>] {
    match statement {
        Statement::BlockStatement(block) => &block.body,
        other => std::slice::from_ref(other),
    }
}

/// The flag `if`: names, the tested path and the second link's text.
struct Flag<'e, 'a> {
    flag: SymbolId,
    temp: SymbolId,
    path: &'e Expression<'a>,
    link: String,
}

/// A read `z ? void 0 : t…` of the pair.
struct Use {
    whole: Span,
    test: Span,
    base: Span,
    rest: String,
}

impl<'s, 'a> Scan<'s, 'a> {
    fn claimed(&self, span: Span) -> bool {
        self.claimed.iter().any(|claim| claim.start <= span.start && span.end <= claim.end)
    }

    fn symbol(&self, identifier: &IdentifierReference<'_>) -> Option<SymbolId> {
        analysis::identifier_symbol(identifier, self.scoping)
    }

    fn reference_spans(&self, symbol: SymbolId) -> Vec<(Span, bool)> {
        self.scoping
            .get_resolved_references(symbol)
            .map(|reference| (self.nodes.get_node(reference.node_id()).span(), reference.is_write()))
            .collect()
    }

    /// `x == null ? void 0 : x…` → `x?.…`.
    fn simple(&mut self, conditional: &ConditionalExpression<'a>) {
        if self.claimed(conditional.span) || self.chained.contains(&conditional.span) || !is_undefined(&conditional.consequent) {
            return;
        }
        let Some(tested) = null_test(&conditional.test) else {
            return;
        };
        let path = text(self.source, tested.span());
        let alternate = defined_or_undefined(&conditional.alternate, self.source).unwrap_or(&conditional.alternate);
        let Some(base) = chain_link(alternate, self.source, path) else {
            return;
        };
        if !pure_path(tested) {
            return;
        }
        let Some(rest) = link_text(self.source, base.span(), alternate.get_inner_expression().span()) else {
            return;
        };
        self.edits.push(analysis::Edit::replace(conditional.span, format!("{path}{}", optional_link(&rest))));
        self.claimed.push(conditional.span);
        self.count += 1;
    }

    /// `if (p == null) { z = true; t = p; } else { t = p.m; z = t == null; }`
    fn flag<'e>(&self, statement: &'e Statement<'a>) -> Option<Flag<'e, 'a>> {
        let Statement::IfStatement(flag_if) = statement else {
            return None;
        };
        let path = null_test(&flag_if.test)?;
        if !pure_path(path) {
            return None;
        }
        let path_text = text(self.source, path.span());
        let [first, second] = branch(&flag_if.consequent) else {
            return None;
        };
        let [load, test] = branch(flag_if.alternate.as_ref()?) else {
            return None;
        };
        // then: z = true (or the test again); t = p; in either order.
        let (one, two) = (assigned_name(first)?, assigned_name(second)?);
        let is_true = |value: &Expression<'_>| match value.get_inner_expression() {
            Expression::BooleanLiteral(value) => value.value,
            test => null_test(test).is_some_and(|tested| text(self.source, tested.span()) == path_text),
        };
        let ((flag, set), (temp, copied)) = if is_true(one.1) { (one, two) } else { (two, one) };
        if !is_true(set) || text(self.source, copied.get_inner_expression().span()) != path_text {
            return None;
        }
        let (flag, temp) = (self.symbol(flag)?, self.symbol(temp)?);
        // else: t = p…; z = t == null;
        let (loaded, value) = assigned_name(load)?;
        let (tested, compared) = assigned_name(test)?;
        if self.symbol(loaded)? != temp || self.symbol(tested)? != flag || flag == temp {
            return None;
        }
        let Expression::Identifier(compared) = null_test(compared)? else {
            return None;
        };
        if self.symbol(compared)? != temp {
            return None;
        }
        // One link on p: p.m, not p.a.b.
        let value = value.get_inner_expression();
        let base = match value {
            Expression::StaticMemberExpression(member) => member.object.get_inner_expression(),
            Expression::ComputedMemberExpression(member) => member.object.get_inner_expression(),
            _ => return None,
        };
        if text(self.source, base.span()) != path_text || !pure_path(value) {
            return None;
        }
        let link = link_text(self.source, base.span(), value.span())?;
        Some(Flag { flag, temp, path, link })
    }

    fn list(&mut self, statements: &[Statement<'a>]) {
        for (index, statement) in statements.iter().enumerate() {
            if self.claimed(statement.span()) {
                continue;
            }
            let Some(flag) = self.flag(statement) else {
                continue;
            };
            self.rewrite_flag(statements, index, &flag);
        }
    }

    fn rewrite_flag(&mut self, statements: &[Statement<'a>], index: usize, flag: &Flag<'_, 'a>) -> Option<()> {
        let flag_if = statements[index].span();
        // Both names declared together, uninitialised, earlier in this list.
        let declaration_of = |symbol: SymbolId| {
            let declarator = self.scoping.symbol_declaration(symbol);
            match self.nodes.parent_node(declarator).kind() {
                oxc_ast::AstKind::VariableDeclaration(declaration) => Some(declaration),
                _ => None,
            }
        };
        let declaration = declaration_of(flag.flag)?;
        if declaration_of(flag.temp)?.span != declaration.span
            || declaration.kind != VariableDeclarationKind::Let
            || declaration.declarations.iter().any(|declarator| declarator.init.is_some())
            || !statements[..index].iter().any(|statement| statement.span() == declaration.span)
        {
            return None;
        }
        let flag_refs = self.reference_spans(flag.flag);
        let temp_refs = self.reference_spans(flag.temp);
        let inside = |span: Span, outer: Span| outer.start <= span.start && span.end <= outer.end;
        // Nothing reads or writes either name before the `if`.
        if flag_refs.iter().chain(&temp_refs).any(|(span, _)| span.start < flag_if.start) {
            return None;
        }
        // The pair's reads after the `if`, outside nested functions.
        let rest = &statements[index + 1..];
        let mut finder = Uses { scan: self, flag: flag.flag, temp: flag.temp, depth: 0, found: Vec::new() };
        for statement in rest {
            finder.visit_statement(statement);
        }
        let uses = finder.found;
        let tests: HashMap<Span, usize> = uses.iter().enumerate().map(|(at, found)| (found.test, at)).collect();
        let bases: HashSet<Span> = uses.iter().map(|found| found.base).collect();
        let mut walk = FlagWalk { flag_refs: &flag_refs, temp_refs: &temp_refs, tests: &tests, bases: &bases, used: HashSet::new() };
        let written = walk.statements(rest)?;
        let used = walk.used;
        // Every other reference: the `if`'s own, or of `z` after its new
        // value on every path.
        if temp_refs.iter().any(|(span, _)| !inside(*span, flag_if) && !used.iter().any(|&at| uses[at].base == *span)) {
            return None;
        }
        let flag_kept = flag_refs.iter().any(|(span, _)| !inside(*span, flag_if) && !tests.get(span).is_some_and(|at| used.contains(at)));
        if flag_kept && !written {
            return None;
        }
        // The path reads the same up to the last use.
        let last = used.iter().map(|&at| uses[at].whole.end).max()?;
        let region = &self.source[flag_if.end as usize..last as usize];
        let mut names = PathNames::default();
        names.visit_expression(flag.path);
        for &reference in &names.references {
            if let Some(symbol) = self.scoping.get_reference(reference).symbol_id() {
                if self.reference_spans(symbol).iter().any(|(span, write)| *write && span.start >= flag_if.end && span.end <= last) {
                    return None;
                }
            }
        }
        for property in &names.properties {
            let mut from = 0;
            while let Some(at) = region[from..].find(property.as_str()) {
                let after = region[from + at + property.len()..].trim_start();
                if after.starts_with('=') && !after.starts_with("==") || ["+=", "-=", "|=", "&=", "^=", "*=", "/=", "%=", "++", "--", "<<=", ">>=", "**="].iter().any(|op| after.starts_with(op)) {
                    return None;
                }
                from += at + property.len();
            }
        }

        let path_text = text(self.source, flag.path.span());
        let head = format!("{path_text}{}", optional_link(&flag.link));
        for &at in &used {
            let found = &uses[at];
            self.edits.push(analysis::Edit::replace(found.whole, format!("{head}{}", optional_link(&found.rest))));
            self.claimed.push(found.whole);
        }
        self.edits.push(analysis::Edit::delete(whole_line(self.source, flag_if)));
        self.claimed.push(flag_if);
        let kept: Vec<_> = declaration
            .declarations
            .iter()
            .filter_map(|declarator| declarator.id.get_binding_identifier())
            .filter(|binding| {
                let symbol = binding.symbol_id();
                symbol != flag.temp && (symbol != flag.flag || flag_kept)
            })
            .map(|binding| binding.name.as_str())
            .collect();
        if kept.is_empty() {
            self.edits.push(analysis::Edit::delete(whole_line(self.source, declaration.span)));
        } else {
            self.edits.push(analysis::Edit::replace(declaration.span, format!("let {};", kept.join(", "))));
        }
        self.claimed.push(declaration.span);
        self.count += 1;
        Some(())
    }
}

/// Finds `z ? void 0 : t…` reads outside nested functions.
struct Uses<'r, 's, 'a> {
    scan: &'r Scan<'s, 'a>,
    flag: SymbolId,
    temp: SymbolId,
    depth: usize,
    found: Vec<Use>,
}

impl<'a> Visit<'a> for Uses<'_, '_, 'a> {
    fn visit_function(&mut self, function: &Function<'a>, flags: oxc_syntax::scope::ScopeFlags) {
        self.depth += 1;
        walk::walk_function(self, function, flags);
        self.depth -= 1;
    }
    fn visit_arrow_function_expression(&mut self, arrow: &ArrowFunctionExpression<'a>) {
        self.depth += 1;
        walk::walk_arrow_function_expression(self, arrow);
        self.depth -= 1;
    }
    fn visit_conditional_expression(&mut self, conditional: &ConditionalExpression<'a>) {
        walk::walk_conditional_expression(self, conditional);
        if self.depth > 0 || self.scan.chained.contains(&conditional.span) || !is_undefined(&conditional.consequent) {
            return;
        }
        let Expression::Identifier(test) = conditional.test.get_inner_expression() else {
            return;
        };
        let Some(Expression::Identifier(base)) = chain_base(&conditional.alternate) else {
            return;
        };
        if self.scan.symbol(test) != Some(self.flag) || self.scan.symbol(base) != Some(self.temp) {
            return;
        }
        let Some(rest) = link_text(self.scan.source, base.span, conditional.alternate.get_inner_expression().span()) else {
            return;
        };
        self.found.push(Use { whole: conditional.span, test: test.span, base: base.span, rest });
    }
}

/// Walks statements while `z` still holds the flag.
struct FlagWalk<'r> {
    flag_refs: &'r [(Span, bool)],
    temp_refs: &'r [(Span, bool)],
    tests: &'r HashMap<Span, usize>,
    bases: &'r HashSet<Span>,
    used: HashSet<usize>,
}

impl FlagWalk<'_> {
    /// Every reference of the pair in `span`, all being reads of the pair:
    /// `Some(uses)`; otherwise `None`.
    fn reads_only(&self, span: Span, except: Option<Span>) -> Option<Vec<usize>> {
        let within = |at: &Span| span.start <= at.start && at.end <= span.end && Some(*at) != except;
        let mut uses = Vec::new();
        for (at, _) in self.flag_refs.iter().filter(|(at, _)| within(at)) {
            uses.push(*self.tests.get(at)?);
        }
        for (at, _) in self.temp_refs.iter().filter(|(at, _)| within(at)) {
            if !self.bases.contains(at) {
                return None;
            }
        }
        Some(uses)
    }

    /// `Some(true)`: `z` is assigned on every path; `Some(false)`: the list
    /// ends with `z` still the flag; `None`: some reference doesn't fit.
    fn statements(&mut self, statements: &[Statement<'_>]) -> Option<bool> {
        for statement in statements {
            if let Some(uses) = self.reads_only(statement.span(), None) {
                self.used.extend(uses);
                continue;
            }
            match statement {
                Statement::ExpressionStatement(expression) => {
                    let Expression::AssignmentExpression(assignment) = &expression.expression else {
                        return None;
                    };
                    let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else {
                        return None;
                    };
                    if assignment.operator != AssignmentOperator::Assign || !self.flag_refs.iter().any(|(at, write)| *write && *at == target.span) {
                        return None;
                    }
                    let uses = self.reads_only(assignment.right.span(), None)?;
                    self.used.extend(uses);
                    return Some(true);
                }
                Statement::IfStatement(branching) => {
                    let uses = self.reads_only(branching.test.span(), None)?;
                    let alternate = branching.alternate.as_ref()?;
                    if self.statements(branch(&branching.consequent))? && self.statements(branch(alternate))? {
                        self.used.extend(uses);
                        return Some(true);
                    }
                    return None;
                }
                _ => return None,
            }
        }
        Some(false)
    }
}

/// The names and property keys a path reads.
#[derive(Default)]
struct PathNames {
    references: Vec<oxc_semantic::ReferenceId>,
    properties: Vec<String>,
}

impl<'a> Visit<'a> for PathNames {
    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        self.references.push(identifier.reference_id());
    }
    fn visit_static_member_expression(&mut self, member: &StaticMemberExpression<'a>) {
        self.properties.push(format!(".{}", member.property.name));
        walk::walk_static_member_expression(self, member);
    }
    fn visit_computed_member_expression(&mut self, member: &ComputedMemberExpression<'a>) {
        match &member.expression {
            Expression::StringLiteral(key) => self.properties.push(format!("[\"{}\"]", key.value)),
            Expression::NumericLiteral(key) => self.properties.push(format!("[{}]", key.value)),
            _ => {}
        }
        walk::walk_computed_member_expression(self, member);
    }
}

impl<'a> Visit<'a> for Scan<'_, 'a> {
    fn visit_function_body(&mut self, body: &FunctionBody<'a>) {
        self.list(&body.statements);
        walk::walk_function_body(self, body);
    }
    fn visit_block_statement(&mut self, block: &BlockStatement<'a>) {
        self.list(&block.body);
        walk::walk_block_statement(self, block);
    }
    fn visit_program(&mut self, program: &Program<'a>) {
        self.list(&program.body);
        walk::walk_program(self, program);
    }
    fn visit_conditional_expression(&mut self, conditional: &ConditionalExpression<'a>) {
        self.simple(conditional);
        walk::walk_conditional_expression(self, conditional);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_tests_become_optional_links() {
        let source = "function f(e) {\n\tlet a = e[\"stack\"] == null ? void 0 : e[\"stack\"][\"toString\"]();\n\tlet b = e == null ? void 0 : e.x;\n\treturn a + b;\n}\n";
        let (code, count) = optional_chains(source).unwrap();
        assert_eq!(count, 2, "{code}");
        assert_eq!(code, "function f(e) {\n\tlet a = e[\"stack\"]?.[\"toString\"]();\n\tlet b = e?.x;\n\treturn a + b;\n}\n");
    }

    #[test]
    fn keeps_called_or_mismatched_conditionals() {
        for source in [
            "function f(e) {\n\treturn (e == null ? void 0 : e.x)();\n}\n",
            "function f(e, g) {\n\treturn e == null ? void 0 : g.x;\n}\n",
            "function f(e) {\n\treturn e() == null ? void 0 : e().x;\n}\n",
        ] {
            let (_, count) = optional_chains(source).unwrap();
            assert_eq!(count, 0, "{source}");
        }
    }

    #[test]
    fn flag_pairs_become_two_link_chains() {
        let source = "function f(w, k) {\n\tlet z, t;\n\tif (w[\"P\"] == null) {\n\t\tz = w[\"P\"] == null;\n\t\tt = w[\"P\"];\n\t} else {\n\t\tt = (w[\"P\"])[\"prototype\"];\n\t\tz = t == null;\n\t}\n\tif (!(z ? void 0 : t[\"c\"])) {\n\t\tz = false;\n\t} else {\n\t\tz = k(z ? void 0 : t[\"c\"]);\n\t}\n\treturn z;\n}\n";
        let (code, count) = optional_chains(source).unwrap();
        assert_eq!(count, 1);
        assert_eq!(code, "function f(w, k) {\n\tlet z;\n\tif (!(w[\"P\"]?.[\"prototype\"]?.[\"c\"])) {\n\t\tz = false;\n\t} else {\n\t\tz = k(w[\"P\"]?.[\"prototype\"]?.[\"c\"]);\n\t}\n\treturn z;\n}\n");
    }

    #[test]
    fn keeps_flag_pairs_read_otherwise() {
        for source in [
            // z read as itself while still the flag
            "function f(w) {\n\tlet z, t;\n\tif (w[\"P\"] == null) {\n\t\tz = true;\n\t\tt = w[\"P\"];\n\t} else {\n\t\tt = w[\"P\"][\"prototype\"];\n\t\tz = t == null;\n\t}\n\treturn z;\n}\n",
            // only one branch reassigns z
            "function f(w, k) {\n\tlet z, t;\n\tif (w[\"P\"] == null) {\n\t\tz = true;\n\t\tt = w[\"P\"];\n\t} else {\n\t\tt = w[\"P\"][\"prototype\"];\n\t\tz = t == null;\n\t}\n\tif (k) {\n\t\tz = 1;\n\t}\n\treturn z;\n}\n",
            // the path changes in between
            "function f(w, k) {\n\tlet z, t;\n\tif (w[\"P\"] == null) {\n\t\tz = true;\n\t\tt = w[\"P\"];\n\t} else {\n\t\tt = w[\"P\"][\"prototype\"];\n\t\tz = t == null;\n\t}\n\tw[\"P\"] = k;\n\treturn z ? void 0 : t[\"c\"];\n}\n",
        ] {
            let (_, count) = optional_chains(source).unwrap();
            assert_eq!(count, 0, "{source}");
        }
    }
    #[test]
    fn parenthesised_and_typeof_guarded_null_tests() {
        let source = "function f() {\n\tlet a;\n\ta = (null == (k[\"c\"])) ? (void 0) : ((k[\"c\"])[\"g\"](new U(12)));\n\treturn a;\n}\n";
        let (code, count) = optional_chains(source).unwrap();
        assert_eq!(count, 1, "{code}");
        assert_eq!(code, "function f() {\n\tlet a;\n\ta = k[\"c\"]?.[\"g\"](new U(12));\n\treturn a;\n}\n");
        let source = "function f(e) {\n\treturn e[\"t\"] == null ? void 0 : typeof e[\"t\"][\"r\"] !== \"undefined\" ? e[\"t\"][\"r\"] : void 0;\n}\n";
        let (code, _) = optional_chains(source).unwrap();
        assert_eq!(code, "function f(e) {\n\treturn e[\"t\"]?.[\"r\"];\n}\n");
    }
}
