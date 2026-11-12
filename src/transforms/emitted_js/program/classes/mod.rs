//! Rebuild `class` declarations from the constructor and method threads
//! the compiled source split them into.
//!
//! TypeScript and Babel lower a class to a strict constructor function and
//! one function per method, stored on its prototype or itself:
//!
//! ```js
//! function C() { "use strict"; this["a"] = 1; }
//! function M() { "use strict"; return this["a"]; }
//! C["prototype"]["get"] = M;
//! function S() { "use strict"; return new C(); }
//! C["make"] = S;
//! // →
//! class C {
//!     constructor() { this["a"] = 1; }
//!     get() { return this["a"]; }
//!     static make() { return new C(); }
//! }
//! ```
//!
//! This runs on the whole program: the methods are separate threads. The
//! class takes the constructor's place, and a member store joins it when
//! nothing between the two could see the class: no call, no `new` and no
//! read of it run there. Every other use of the class, through any variable
//! it is copied to (a factory's result included), is `new`, a member read or
//! store, a copy or a return: a class cannot be called, so a constructor that
//! is called or handed to other code stays a function. No use may run before
//! the class, which unlike a function declaration is not hoisted, and none
//! may sit outside the statement list declaring it. Each moved function is
//! strict (or runs the same when strict) and used only by its store;
//! `constructor`, `prototype`, `name` and `length` stay stores.
//!
//! Subclasses come in two lowerings. TypeScript's constructor is `return
//! S.apply(this, arguments) || this`, linked by `C.prototype =
//! Object.create(S.prototype)`, `C.prototype.constructor = C` and
//! `extendStatics(C, S)`; it becomes `class C extends S` with
//! `super(...arguments)`, and `S.prototype.m.call(this, …)` in its methods
//! `super.m(…)`. Babel's checks `this instanceof C`, calls
//! `getPrototypeOf(C).call(this, …)` and returns the result; its `_inherits`
//! block and the descriptor arrays handed to `_createClass` give the rest.
//! A class handed to a thread is followed into the thread's `arguments[n]`;
//! one called through `apply`/`call`/`bind` stays a function, so the pass
//! repeats until a base's subclasses no longer call it.

use std::collections::HashSet;

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast::AstKind;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, NodeId, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};

use crate::analysis;
use super::super::support::whole_line;

mod members;
mod build;
mod babel;
#[cfg(test)]
mod tests;


pub(crate) fn rebuild_classes(source: &str) -> Result<(String, usize), String> {
    // A base class converts once its subclass no longer calls it.
    let mut code = source.to_string();
    let mut total = 0;
    for _ in 0..4 {
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
    if !source.contains("[\"prototype\"]") {
        return Ok((source.into(), 0));
    }
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut scan = Scan { source, scoping: &scoping, nodes: &nodes, edits: Vec::new(), count: 0 };
    scan.visit_program(&program);
    if scan.count == 0 {
        return Ok((source.into(), 0));
    }
    let output = super::super::support::apply_checked(source, scan.edits, "class")?;
    Ok((output, scan.count))
}

struct Scan<'s, 'a> {
    source: &'s str,
    scoping: &'s Scoping,
    nodes: &'s AstNodes<'a>,
    edits: Vec<analysis::Edit>,
    count: usize,
}

fn strict(function: &Function<'_>) -> bool {
    function.body.as_ref().is_some_and(|body| body.directives.iter().any(|directive| directive.directive == "use strict"))
}

/// A sloppy function that runs the same in strict mode: nothing in it
/// (nested functions included) reads `this`, assigns an undeclared name,
/// reads `arguments.callee`/`caller`, deletes a name, uses `with`/`eval`,
/// binds `eval` or `arguments`, or reads the `arguments` of a function with
/// parameters (which it aliases only in sloppy code). Parameters themselves
/// are plain, distinct names: a default, a pattern, a rest or a repeated
/// name is refused. A function declared in a block is read only inside it:
/// sloppy code also binds it in the enclosing function.
#[derive(Default)]
struct SloppyOnly {
    found: bool,
    unresolved_writes: Vec<oxc_semantic::ReferenceId>,
    /// The innermost non-arrow function around the visit declares
    /// parameters, so its `arguments` (an arrow's too) aliases them.
    mapped: bool,
    /// The blocks around the visit, inside the innermost function.
    blocks: Vec<Span>,
    /// Functions declared in a block, and the block.
    block_functions: Vec<(String, Span)>,
    references: Vec<(String, Span)>,
}

impl SloppyOnly {
    /// Nothing found, and every function declared in a block read only
    /// inside it.
    fn clean(&self) -> bool {
        !self.found
            && self.block_functions.iter().all(|(name, block)| {
                self.references.iter().all(|(read, span)| read != name || block.contains_inclusive(*span))
            })
    }

    fn in_block(&mut self, span: Span, walk: impl FnOnce(&mut Self)) {
        self.blocks.push(span);
        walk(self);
        self.blocks.pop();
    }
}

/// `(a, b, c)`: plain names, none repeated.
fn plain_parameters(parameters: &FormalParameters<'_>) -> bool {
    let mut names = HashSet::new();
    parameters.rest.is_none()
        && parameters.items.iter().all(|parameter| {
            parameter.initializer.is_none() && matches!(&parameter.pattern, BindingPattern::BindingIdentifier(id) if names.insert(id.name.as_str()))
        })
}

impl<'a> Visit<'a> for SloppyOnly {
    fn visit_this_expression(&mut self, _: &ThisExpression) {
        self.found = true;
    }
    fn visit_with_statement(&mut self, _: &WithStatement<'a>) {
        self.found = true;
    }
    fn visit_function(&mut self, function: &Function<'a>, flags: oxc_syntax::scope::ScopeFlags) {
        if let (true, Some(id), Some(&block)) = (function.is_declaration(), &function.id, self.blocks.last()) {
            self.block_functions.push((id.name.to_string(), block));
        }
        let blocks = std::mem::take(&mut self.blocks);
        let outer = self.mapped;
        self.mapped = !function.params.items.is_empty() || function.params.rest.is_some();
        if self.mapped && !plain_parameters(&function.params) {
            self.found = true;
        }
        walk::walk_function(self, function, flags);
        self.mapped = outer;
        self.blocks = blocks;
    }
    fn visit_arrow_function_expression(&mut self, arrow: &ArrowFunctionExpression<'a>) {
        let blocks = std::mem::take(&mut self.blocks);
        walk::walk_arrow_function_expression(self, arrow);
        self.blocks = blocks;
    }
    fn visit_block_statement(&mut self, block: &BlockStatement<'a>) {
        self.in_block(block.span, |this| walk::walk_block_statement(this, block));
    }
    fn visit_switch_statement(&mut self, switch: &SwitchStatement<'a>) {
        self.in_block(switch.span, |this| walk::walk_switch_statement(this, switch));
    }
    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        self.references.push((identifier.name.to_string(), identifier.span));
        if identifier.name == "eval" || identifier.name == "arguments" && self.mapped {
            self.found = true;
        }
    }
    fn visit_binding_identifier(&mut self, identifier: &BindingIdentifier<'a>) {
        if matches!(identifier.name.as_str(), "eval" | "arguments") {
            self.found = true;
        }
    }
    fn visit_simple_assignment_target(&mut self, target: &SimpleAssignmentTarget<'a>) {
        if let SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier) = target {
            // Resolved later: an unresolved write is an implicit global.
            self.unresolved_writes.push(identifier.reference_id());
        }
        walk::walk_simple_assignment_target(self, target);
    }
    fn visit_unary_expression(&mut self, unary: &UnaryExpression<'a>) {
        if unary.operator == oxc_syntax::operator::UnaryOperator::Delete && matches!(unary.argument.get_inner_expression(), Expression::Identifier(_)) {
            self.found = true;
        }
        walk::walk_unary_expression(self, unary);
    }
    fn visit_member_expression(&mut self, member: &MemberExpression<'a>) {
        if matches!(member.object().get_inner_expression(), Expression::Identifier(object) if object.name == "arguments")
            && member_key(member).is_some_and(|(_, key)| key == "callee" || key == "caller")
        {
            self.found = true;
        }
        walk::walk_member_expression(self, member);
    }
}

fn thread<'e, 'a>(statement: &'e Statement<'a>) -> Option<&'e Function<'a>> {
    match statement {
        Statement::FunctionDeclaration(function)
            if function.id.as_ref().is_some_and(|id| id.name.starts_with("$THREAD_"))
                && !function.generator
                && !function.r#async =>
        {
            Some(function)
        }
        _ => None,
    }
}

fn literal_key(expression: &Expression<'_>) -> Option<String> {
    match expression {
        Expression::StringLiteral(key) => Some(key.value.to_string()),
        _ => None,
    }
}

/// `o["k"]` / `o.k`: the object and key.
fn member_key<'e, 'a>(member: &'e MemberExpression<'a>) -> Option<(&'e Expression<'a>, String)> {
    match member {
        MemberExpression::StaticMemberExpression(member) if !member.optional => Some((&member.object, member.property.name.to_string())),
        MemberExpression::ComputedMemberExpression(member) if !member.optional => Some((&member.object, literal_key(&member.expression)?)),
        _ => None,
    }
}

/// A member store `H["prototype"]["k"] = M;` or `H["k"] = M;`.
struct Store {
    holder: SymbolId,
    instance: bool,
    key: String,
    value: SymbolId,
}

/// A class element: the function declared at `function` in the list.
struct Member {
    function: usize,
    key: String,
    instance: bool,
    /// `""`, `"get "` or `"set "`.
    kind: &'static str,
}

/// Calls, `new`, tagged templates: code that could run and see the class.
#[derive(Default)]
struct Runs {
    found: bool,
}

impl<'a> Visit<'a> for Runs {
    fn visit_call_expression(&mut self, _: &CallExpression<'a>) {
        self.found = true;
    }
    fn visit_new_expression(&mut self, _: &NewExpression<'a>) {
        self.found = true;
    }
    fn visit_tagged_template_expression(&mut self, _: &TaggedTemplateExpression<'a>) {
        self.found = true;
    }
    fn visit_function(&mut self, _: &Function<'a>, _: oxc_syntax::scope::ScopeFlags) {}
    fn visit_arrow_function_expression(&mut self, _: &ArrowFunctionExpression<'a>) {}
}

/// A `_createClass` element: key, `""`/`"get "`/`"set "`, on the
/// prototype (not static), and the function.
type Descriptor = (String, &'static str, bool, SymbolId);

/// Text without whitespace, for comparing statements with a template.
fn squeeze(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}

/// Removes one leading tab from every line after the first.
fn dedent(text: &str) -> String {
    text.split('\n').enumerate().map(|(at, line)| if at == 0 { line } else { line.strip_prefix('\t').unwrap_or(line) }).collect::<Vec<_>>().join("\n")
}

fn text(source: &str, span: Span) -> &str {
    &source[span.start as usize..span.end as usize]
}

impl<'a> Visit<'a> for Scan<'_, 'a> {
    fn visit_function_body(&mut self, body: &FunctionBody<'a>) {
        self.list(&body.statements, body.span);
        walk::walk_function_body(self, body);
    }
    fn visit_block_statement(&mut self, block: &BlockStatement<'a>) {
        self.list(&block.body, block.span);
        walk::walk_block_statement(self, block);
    }
    fn visit_program(&mut self, program: &Program<'a>) {
        self.list(&program.body, program.span);
        walk::walk_program(self, program);
    }
}

