//! Which generated values are primitives, and which expressions are pure.
//!
//! Every JavaScript operator returns a primitive, and an operator over
//! primitive operands runs no user code: there is no `valueOf`/`toString`
//! to call. This module types never-written bindings from their
//! initializers, in source order, as Number, String, Boolean or nullish:
//!
//! - Number: numeric literals, `NaN`/`Infinity`, unary `+`, `Number(…)`,
//!   `parseInt`/`parseFloat`, `Math` method calls (standard built-ins), and
//!   arithmetic, bitwise and shift operators with at least one operand of a
//!   known primitive type (a BigInt mixed with a number throws, so such an
//!   expression that completes produced a number);
//! - String: string literals, `typeof`, `String(…)`, `+` with a string
//!   operand;
//! - Boolean: comparisons, equality, `!`, `in`, `instanceof`, `Boolean(…)`,
//!   `isNaN`/`isFinite`;
//! - nullish: `null`, `void e`, `undefined`.
//!
//! A rewritten `let` keeps its initializer's type when every write stores
//! that type, and a plain parameter of a function only ever called directly
//! (no spread, no `arguments`) takes the type every call passes; both are
//! assumed and then checked together until no assumption drops.
//!
//! An expression is *pure* — it may be dropped, moved, or evaluated where it
//! was not — when it has no effect and cannot throw: literals, initialized
//! names, `!`/`typeof`/`void`/`===`/`!==`/`&&`/`||`/`??`/`?:` over pure
//! operands, and the other operators only over pure operands of known
//! primitive type, and named properties of standard built-ins. Other member
//! reads, calls, `in`, `instanceof` and assignments are never pure. A name is
//! initialized when it is a `var`, a plain parameter, a function's leading
//! `let a, b;`, a hoisted function, or a declaration earlier in the same
//! function.

use std::collections::{BTreeMap, BTreeSet};

use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{Scoping, SymbolId};
use oxc_syntax::operator::{BinaryOperator, LogicalOperator, UnaryOperator};

use crate::analysis;

/// `expression` creates an object (array, object, function or class literal):
/// copying its text would split one identity into several.
pub(crate) fn allocates(expression: &Expression<'_>) -> bool {
    struct Finder(bool);
    impl<'a> Visit<'a> for Finder {
        fn visit_expression(&mut self, expression: &Expression<'a>) {
            if matches!(
                expression,
                Expression::ArrayExpression(_)
                    | Expression::ObjectExpression(_)
                    | Expression::FunctionExpression(_)
                    | Expression::ArrowFunctionExpression(_)
                    | Expression::ClassExpression(_)
                    | Expression::NewExpression(_)
                    | Expression::RegExpLiteral(_)
                    // The only calls `pure` accepts are factories, which
                    // build something fresh.
                    | Expression::CallExpression(_)
            ) {
                self.0 = true;
            } else if !self.0 {
                walk::walk_expression(self, expression);
            }
        }
    }
    let mut finder = Finder(false);
    finder.visit_expression(expression);
    finder.0
}

/// `Object["defineProperty"]`, `Math.abs`, `Function.prototype.call`: a named
/// property of an unresolved standard namespace or constructor, or of such a
/// property, reads a fixed value under the standard built-ins assumption.
/// `RegExp` and `Error` carry changing statics (`RegExp.$1`,
/// `Error.stackTraceLimit`) and are left out.
pub(crate) fn is_builtin_member(member: &MemberExpression<'_>, scoping: &Scoping) -> bool {
    let (object, optional) = match member {
        MemberExpression::StaticMemberExpression(member) => (&member.object, member.optional),
        MemberExpression::ComputedMemberExpression(member) => {
            if !matches!(member.expression, Expression::StringLiteral(_)) {
                return false;
            }
            (&member.object, member.optional)
        }
        MemberExpression::PrivateFieldExpression(_) => return false,
    };
    // `Function.prototype.toString`: a named property of a fixed built-in
    // value is fixed too.
    if let Some(inner) = object.as_member_expression() {
        return !optional && is_builtin_member(inner, scoping);
    }
    let Expression::Identifier(object) = object else {
        return false;
    };
    !optional
        && object
            .reference_id
            .get()
            .is_some_and(|reference| scoping.get_reference(reference).symbol_id().is_none())
        && matches!(
            object.name.as_str(),
            "Object" | "Function" | "Array" | "Number" | "String" | "Boolean" | "Symbol" | "Math" | "JSON"
                | "Reflect" | "Promise" | "Date" | "ArrayBuffer" | "Uint8Array" | "BigInt"
        )
}

/// Global names that read as fixed values under the standard built-ins
/// assumption: the ECMAScript constructors, namespaces and functions, plus
/// the non-writable `undefined`, `NaN` and `Infinity`. Host (DOM) globals
/// are not included.
pub(crate) fn is_stable_global(name: &str) -> bool {
    matches!(
        name,
        "undefined" | "NaN" | "Infinity" | "globalThis"
            | "Object" | "Function" | "Array" | "Number" | "String" | "Boolean" | "Symbol" | "BigInt"
            | "Math" | "JSON" | "Date" | "RegExp" | "Promise" | "Proxy" | "Reflect" | "Intl" | "Atomics"
            | "Error" | "TypeError" | "RangeError" | "SyntaxError" | "ReferenceError" | "EvalError" | "URIError"
            | "Map" | "Set" | "WeakMap" | "WeakSet" | "ArrayBuffer" | "SharedArrayBuffer" | "DataView"
            | "Int8Array" | "Uint8Array" | "Uint8ClampedArray" | "Int16Array" | "Uint16Array"
            | "Int32Array" | "Uint32Array" | "Float32Array" | "Float64Array" | "BigInt64Array" | "BigUint64Array"
            | "parseInt" | "parseFloat" | "isNaN" | "isFinite" | "encodeURIComponent" | "decodeURIComponent"
            | "encodeURI" | "decodeURI" | "escape" | "unescape"
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Prim {
    Number,
    String,
    Boolean,
    Nullish,
}

pub(crate) struct Types<'s> {
    scoping: &'s Scoping,
    bindings: BTreeMap<SymbolId, Prim>,
    /// Where each singly declared binding's declaration ends.
    declared: BTreeMap<SymbolId, u32>,
    /// Functions (and names stored once with one) whose body only builds
    /// values of its own and returns one: a call runs no outside code.
    factories: BTreeSet<SymbolId>,
}

/// Function declarations whose body only declares and stores its own
/// locals with pure values, fresh literals or `Object.create(…)`, declares
/// functions, and returns.
struct Factories<'t, 's> {
    types: &'t Types<'s>,
    found: BTreeSet<SymbolId>,
}

impl<'a> Visit<'a> for Factories<'_, '_> {
    fn visit_function(&mut self, function: &Function<'a>, flags: oxc_syntax::scope::ScopeFlags) {
        if let (Some(id), Some(body)) = (&function.id, &function.body) {
            // A thread's body here is not its code: while threads are
            // cleaned one by one, each child is an empty stub the linker
            // fills in later, and calling it can run anything.
            if !id.name.starts_with("$THREAD_")
                && !function.r#async
                && !function.generator
                && self.factory_body(&body.statements, function.span)
            {
                if let Some(symbol) = id.symbol_id.get() {
                    self.found.insert(symbol);
                }
            }
        }
        walk::walk_function(self, function, flags);
    }
}

impl Factories<'_, '_> {
    fn value(&self, expression: &Expression<'_>) -> bool {
        match expression.without_parentheses() {
            Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_) => true,
            Expression::ArrayExpression(array) => array.elements.iter().all(|e| e.as_expression().is_some_and(|e| self.value(e))),
            Expression::ObjectExpression(object) => object.properties.iter().all(|p| match p {
                ObjectPropertyKind::ObjectProperty(p) => !p.computed && self.value(&p.value),
                _ => false,
            }),
            Expression::ComputedMemberExpression(member) => {
                matches!(&member.object, Expression::Identifier(o) if o.name == "arguments")
                    && matches!(member.expression, Expression::NumericLiteral(_))
            }
            Expression::CallExpression(call) => {
                !call.optional
                    && call.callee.as_member_expression().is_some_and(|m| {
                        is_builtin_member(m, self.types.scoping)
                            && m.static_property_name().is_some_and(|n| n == "create")
                    })
                    && call.arguments.iter().all(|a| a.as_expression().is_some_and(|a| self.value(a)))
            }
            expression => self.types.pure(expression),
        }
    }

    /// A binding declared inside the function (its top-level statements
    /// can only name its own).
    fn own(&self, name: &IdentifierReference<'_>, function: oxc_span::Span) -> bool {
        self.types.symbol(name).is_some_and(|symbol| {
            let span = self.types.scoping.symbol_span(symbol);
            function.start <= span.start && span.end <= function.end
        })
    }

    fn factory_body(&self, statements: &[Statement<'_>], function: oxc_span::Span) -> bool {
        statements.iter().all(|statement| match statement {
            Statement::FunctionDeclaration(_) | Statement::EmptyStatement(_) => true,
            Statement::VariableDeclaration(declaration) => declaration
                .declarations
                .iter()
                .all(|d| matches!(d.id, BindingPattern::BindingIdentifier(_)) && d.init.as_ref().is_none_or(|init| self.value(init))),
            Statement::ExpressionStatement(expression) => match &expression.expression {
                Expression::AssignmentExpression(assignment) => {
                    assignment.operator == AssignmentOperator::Assign
                        && matches!(&assignment.left, AssignmentTarget::AssignmentTargetIdentifier(target) if self.own(target, function))
                        && self.value(&assignment.right)
                }
                _ => false,
            },
            Statement::ReturnStatement(ret) => ret.argument.as_ref().is_none_or(|argument| self.value(argument)),
            _ => false,
        })
    }
}

/// Names stored exactly once, with a factory function (`$KEY_n = F;`).
fn factory_aliases(program: &Program<'_>, scoping: &Scoping, factories: &BTreeSet<SymbolId>) -> BTreeSet<SymbolId> {
    struct Aliases<'s> {
        scoping: &'s Scoping,
        factories: &'s BTreeSet<SymbolId>,
        found: BTreeSet<SymbolId>,
    }
    impl<'a> Visit<'a> for Aliases<'_> {
        fn visit_assignment_expression(&mut self, assignment: &AssignmentExpression<'a>) {
            if let (AssignmentTarget::AssignmentTargetIdentifier(target), Expression::Identifier(value)) = (&assignment.left, &assignment.right) {
                let target = self.scoping.get_reference(target.reference_id()).symbol_id();
                let value = self.scoping.get_reference(value.reference_id()).symbol_id();
                if let (Some(target), Some(value)) = (target, value) {
                    let writes = self.scoping.get_resolved_references(target).filter(|r| r.is_write()).count();
                    let declared_plain = self.scoping.symbol_declarations(target).count() == 1;
                    if assignment.operator == AssignmentOperator::Assign && self.factories.contains(&value) && writes == 1 && declared_plain {
                        self.found.insert(target);
                    }
                }
            }
            walk::walk_assignment_expression(self, assignment);
        }
    }
    let mut aliases = Aliases { scoping, factories, found: BTreeSet::new() };
    aliases.visit_program(program);
    aliases.found
}

impl<'s> Types<'s> {
    pub(crate) fn build<'a>(program: &Program<'a>, scoping: &'s Scoping, nodes: &oxc_semantic::AstNodes<'a>) -> Self {
        // The fixpoint below re-checks calls and writes until the types
        // settle: gather those nodes once instead of walking the program
        // each round.
        let mut call_nodes = Vec::new();
        let mut assignments = Vec::new();
        let mut updates = Vec::new();
        for node in nodes.iter() {
            match node.kind() {
                oxc_ast::AstKind::CallExpression(call) => call_nodes.push(call),
                oxc_ast::AstKind::AssignmentExpression(assignment) => assignments.push(assignment),
                oxc_ast::AstKind::UpdateExpression(update) => updates.push(update),
                _ => {}
            }
        }
        let mut types = Types {
            scoping,
            bindings: BTreeMap::new(),
            declared: BTreeMap::new(),
            factories: BTreeSet::new(),
        };
        let mut collect = Collect { types: &mut types, rewritten: BTreeMap::new(), functions: BTreeMap::new() };
        collect.visit_program(program);
        // A rewritten `let` keeps its initializer's type when every write
        // stores that type, and a parameter of a function only ever called
        // directly takes the type every call passes. Lets start assumed
        // typed; parameters gain the type their calls agree on. Then every
        // assumption is checked against its writes and calls, dropping the
        // ones that do not fit (or have a write or use this scan cannot see)
        // until none drop: each value such a binding ever holds then comes
        // from a fitting write or argument over fitting values.
        let mut rewritten = std::mem::take(&mut collect.rewritten);
        let functions = std::mem::take(&mut collect.functions);
        let mut factories = Factories { types: &types, found: BTreeSet::new() };
        factories.visit_program(program);
        let mut found = factories.found;
        // A name stored exactly once, with a factory, calls that factory.
        found.extend(factory_aliases(program, scoping, &found));
        types.factories = found;
        types.bindings.extend(rewritten.iter().map(|(symbol, prim)| (*symbol, *prim)));
        let mut parameters: BTreeMap<SymbolId, Prim> = BTreeMap::new();
        for _ in 0..4 {
            let mut calls = Calls { types: &types, functions: &functions, sites: BTreeMap::new(), escaped: BTreeSet::new() };
            for call in &call_nodes {
                calls.site(call);
            }
            let (all_sites, escaped) = (calls.sites, calls.escaped);
            let mut added = false;
            for (function, params) in &functions {
                let Some(sites) = all_sites.get(function) else {
                    continue;
                };
                if escaped.contains(function) || sites.len() != scoping.get_resolved_references(*function).count() {
                    continue;
                }
                for (index, param) in params.iter().enumerate() {
                    let Some(param) = param else {
                        continue;
                    };
                    let mut agreed = sites.iter().map(|site| site.get(index).copied().unwrap_or(Some(Prim::Nullish)));
                    let Some(Some(first)) = agreed.next() else {
                        continue;
                    };
                    if agreed.all(|other| other == Some(first)) && !parameters.contains_key(param) {
                        parameters.insert(*param, first);
                        types.bindings.insert(*param, first);
                        added = true;
                    }
                }
            }
            if !added {
                break;
            }
        }
        loop {
            let mut check = CheckWrites { types: &types, seen: BTreeMap::new(), failed: BTreeSet::new() };
            for assignment in &assignments {
                check.assignment(assignment);
            }
            for update in &updates {
                check.update(update);
            }
            let mut failed = check.failed;
            for (symbol, _) in &rewritten {
                let writes = scoping.get_resolved_references(*symbol).filter(|reference| reference.is_write()).count();
                if check.seen.get(symbol).copied().unwrap_or(0) != writes {
                    failed.insert(*symbol);
                }
            }
            let mut calls = Calls { types: &types, functions: &functions, sites: BTreeMap::new(), escaped: BTreeSet::new() };
            for call in &call_nodes {
                calls.site(call);
            }
            for (function, params) in &functions {
                let sites = calls.sites.get(function).map_or(&[][..], Vec::as_slice);
                let complete = !calls.escaped.contains(function)
                    && sites.len() == scoping.get_resolved_references(*function).count();
                for (index, param) in params.iter().enumerate() {
                    let Some(param) = param else {
                        continue;
                    };
                    let Some(prim) = parameters.get(param) else {
                        continue;
                    };
                    let fits = complete
                        && sites
                            .iter()
                            .all(|site| site.get(index).copied().unwrap_or(Some(Prim::Nullish)) == Some(*prim));
                    if !fits {
                        failed.insert(*param);
                    }
                }
            }
            failed.retain(|symbol| rewritten.contains_key(symbol) || parameters.contains_key(symbol));
            if failed.is_empty() {
                break;
            }
            for symbol in failed {
                rewritten.remove(&symbol);
                parameters.remove(&symbol);
                types.bindings.remove(&symbol);
            }
        }
        types
    }

    /// The primitive type `expression` evaluates to, when known.
    pub(crate) fn type_of(&self, expression: &Expression<'_>) -> Option<Prim> {
        match expression {
            Expression::NumericLiteral(_) => Some(Prim::Number),
            Expression::StringLiteral(_) => Some(Prim::String),
            Expression::TemplateLiteral(_) => Some(Prim::String),
            Expression::BooleanLiteral(_) => Some(Prim::Boolean),
            Expression::NullLiteral(_) => Some(Prim::Nullish),
            Expression::Identifier(identifier) => match self.symbol(identifier) {
                Some(symbol) => self.bindings.get(&symbol).copied(),
                None => match identifier.name.as_str() {
                    "undefined" => Some(Prim::Nullish),
                    "NaN" | "Infinity" => Some(Prim::Number),
                    _ => None,
                },
            },
            Expression::ParenthesizedExpression(inner) => self.type_of(&inner.expression),
            Expression::SequenceExpression(sequence) => {
                sequence.expressions.last().and_then(|last| self.type_of(last))
            }
            Expression::AssignmentExpression(assignment)
                if assignment.operator == oxc_syntax::operator::AssignmentOperator::Assign =>
            {
                self.type_of(&assignment.right)
            }
            Expression::UnaryExpression(unary) => match unary.operator {
                UnaryOperator::LogicalNot | UnaryOperator::Delete => Some(Prim::Boolean),
                UnaryOperator::Typeof => Some(Prim::String),
                UnaryOperator::Void => Some(Prim::Nullish),
                UnaryOperator::UnaryPlus => Some(Prim::Number),
                UnaryOperator::UnaryNegation | UnaryOperator::BitwiseNot => {
                    self.type_of(&unary.argument).map(|_| Prim::Number)
                }
            },
            Expression::BinaryExpression(binary) => {
                let (left, right) = (self.type_of(&binary.left), self.type_of(&binary.right));
                match binary.operator {
                    BinaryOperator::Equality
                    | BinaryOperator::Inequality
                    | BinaryOperator::StrictEquality
                    | BinaryOperator::StrictInequality
                    | BinaryOperator::LessThan
                    | BinaryOperator::LessEqualThan
                    | BinaryOperator::GreaterThan
                    | BinaryOperator::GreaterEqualThan
                    | BinaryOperator::In
                    | BinaryOperator::Instanceof => Some(Prim::Boolean),
                    BinaryOperator::Addition => match (left, right) {
                        (Some(Prim::String), _) | (_, Some(Prim::String)) => Some(Prim::String),
                        (Some(_), Some(_)) => Some(Prim::Number),
                        _ => None,
                    },
                    BinaryOperator::ShiftRightZeroFill => Some(Prim::Number),
                    // A BigInt mixed with a known primitive throws: completing
                    // means both were non-BigInt.
                    _ => (left.is_some() || right.is_some()).then_some(Prim::Number),
                }
            }
            Expression::LogicalExpression(logical) => {
                let left = self.type_of(&logical.left)?;
                (self.type_of(&logical.right)? == left).then_some(left)
            }
            Expression::ConditionalExpression(conditional) => {
                let consequent = self.type_of(&conditional.consequent)?;
                (self.type_of(&conditional.alternate)? == consequent).then_some(consequent)
            }
            Expression::CallExpression(call) => self.call_type(&call.callee),
            _ => None,
        }
    }

    fn call_type(&self, callee: &Expression<'_>) -> Option<Prim> {
        match callee {
            Expression::Identifier(identifier) if self.symbol(identifier).is_none() => {
                match identifier.name.as_str() {
                    "Number" | "parseInt" | "parseFloat" => Some(Prim::Number),
                    "String" => Some(Prim::String),
                    "Boolean" | "isNaN" | "isFinite" => Some(Prim::Boolean),
                    _ => None,
                }
            }
            Expression::StaticMemberExpression(member) => {
                matches!(&member.object, Expression::Identifier(math)
                    if math.name == "Math" && self.symbol(math).is_none())
                .then_some(Prim::Number)
            }
            Expression::ComputedMemberExpression(member) => {
                matches!((&member.object, &member.expression), (Expression::Identifier(math), Expression::StringLiteral(_))
                    if math.name == "Math" && self.symbol(math).is_none())
                .then_some(Prim::Number)
            }
            _ => None,
        }
    }

    /// Evaluating `expression` has no effect and cannot throw.
    pub(crate) fn pure(&self, expression: &Expression<'_>) -> bool {
        match expression {
            Expression::NumericLiteral(_)
            | Expression::StringLiteral(_)
            | Expression::BooleanLiteral(_)
            | Expression::NullLiteral(_)
            | Expression::FunctionExpression(_)
            | Expression::ArrowFunctionExpression(_) => true,
            Expression::TemplateLiteral(template) => template.expressions.is_empty(),
            Expression::Identifier(identifier) => match self.symbol(identifier) {
                Some(symbol) => self.initialized_at(symbol, identifier),
                None => is_stable_global(&identifier.name),
            },
            // Allocating a literal of pure parts runs nothing.
            Expression::ThisExpression(_) => true,
            Expression::ArrayExpression(array) => array.elements.iter().all(|element| match element {
                ArrayExpressionElement::Elision(_) => true,
                ArrayExpressionElement::SpreadElement(_) => false,
                _ => element.as_expression().is_some_and(|e| self.pure(e)),
            }),
            Expression::ObjectExpression(object) => object.properties.iter().all(|property| match property {
                ObjectPropertyKind::SpreadProperty(_) => false,
                ObjectPropertyKind::ObjectProperty(property) => {
                    (!property.computed
                        || matches!(property.key, PropertyKey::StringLiteral(_) | PropertyKey::NumericLiteral(_)))
                        && self.pure(&property.value)
                }
            }),
            Expression::ParenthesizedExpression(inner) => self.pure(&inner.expression),
            // A factory builds something fresh from its arguments.
            Expression::CallExpression(call) => {
                !call.optional
                    && matches!(&call.callee, Expression::Identifier(callee)
                        if self.symbol(callee).is_some_and(|symbol| self.factories.contains(&symbol)))
                    && call.arguments.iter().all(|a| a.as_expression().is_some_and(|a| self.pure(a)))
            }
            Expression::StaticMemberExpression(_) | Expression::ComputedMemberExpression(_) => {
                expression.as_member_expression().is_some_and(|member| is_builtin_member(member, self.scoping))
            }
            Expression::UnaryExpression(unary) => self.pure_unary(unary),
            Expression::BinaryExpression(binary) => self.pure_binary(binary),
            Expression::LogicalExpression(logical) => {
                matches!(
                    logical.operator,
                    LogicalOperator::And | LogicalOperator::Or | LogicalOperator::Coalesce
                ) && self.pure(&logical.left)
                    && self.pure(&logical.right)
            }
            Expression::ConditionalExpression(conditional) => {
                self.pure(&conditional.test)
                    && self.pure(&conditional.consequent)
                    && self.pure(&conditional.alternate)
            }
            _ => false,
        }
    }

    pub(crate) fn pure_unary(&self, unary: &UnaryExpression<'_>) -> bool {
        match unary.operator {
            UnaryOperator::Typeof => match &unary.argument {
                Expression::Identifier(_) => true,
                argument => self.pure(argument),
            },
            UnaryOperator::LogicalNot | UnaryOperator::Void => self.pure(&unary.argument),
            UnaryOperator::Delete => false,
            // Operands are primitives: the VM's scratch values never carry a
            // `valueOf` (shape/'s redundant-memory rule makes the same call).
            _ => self.pure(&unary.argument),
        }
    }

    /// The operator itself runs no code and cannot throw (its operands are
    /// judged separately by `pure`).
    pub(crate) fn operator_inert(&self, binary: &BinaryExpression<'_>) -> bool {
        match binary.operator {
            BinaryOperator::StrictEquality | BinaryOperator::StrictInequality => true,
            // Comparing with `null` only tests for null and undefined.
            BinaryOperator::Equality | BinaryOperator::Inequality
                if matches!(binary.left, Expression::NullLiteral(_))
                    || matches!(binary.right, Expression::NullLiteral(_)) =>
            {
                true
            }
            BinaryOperator::In | BinaryOperator::Instanceof => false,
            _ => true,
        }
    }

    pub(crate) fn pure_binary(&self, binary: &BinaryExpression<'_>) -> bool {
        self.pure(&binary.left) && self.pure(&binary.right) && self.operator_inert(binary)
    }

    /// The unary operator itself runs no code and cannot throw.
    pub(crate) fn unary_inert(&self, unary: &UnaryExpression<'_>) -> bool {
        match unary.operator {
            UnaryOperator::Typeof | UnaryOperator::LogicalNot | UnaryOperator::Void => true,
            UnaryOperator::Delete => false,
            _ => true,
        }
    }

    /// A binding read after its declaration in the same function: it is
    /// initialized there, so the read cannot throw.
    fn initialized_at(&self, symbol: SymbolId, read: &IdentifierReference<'_>) -> bool {
        // A `var` reads `undefined` before its initializer runs, never throws.
        let flags = self.scoping.symbol_flags(symbol);
        if flags.contains(oxc_semantic::SymbolFlags::FunctionScopedVariable)
            && !self.scoping.symbol_flags(symbol).contains(oxc_semantic::SymbolFlags::Function)
        {
            return true;
        }
        let Some(&end) = self.declared.get(&symbol) else {
            return false;
        };
        // A hoisted function declaration (recorded at 0) is initialized for
        // every function nested in its scope.
        if end == 0 {
            return true;
        }
        let Some(reference) = read.reference_id.get() else {
            return false;
        };
        let read_scope = self.scoping.get_reference(reference).scope_id();
        end <= read.span.start
            && self.function_scope(read_scope) == self.function_scope(self.scoping.symbol_scope_id(symbol))
    }

    fn function_scope(&self, scope: oxc_semantic::ScopeId) -> oxc_semantic::ScopeId {
        let mut current = scope;
        loop {
            if self.scoping.scope_flags(current).is_function() {
                return current;
            }
            match self.scoping.scope_parent_id(current) {
                Some(parent) => current = parent,
                None => return current,
            }
        }
    }

    /// The type of a never-written binding, or of a rewritten `let` whose
    /// every write keeps its initializer's type, when known.
    pub(crate) fn binding_type(&self, symbol: SymbolId) -> Option<Prim> {
        self.bindings.get(&symbol).copied()
    }

    fn symbol(&self, identifier: &IdentifierReference<'_>) -> Option<SymbolId> {
        analysis::identifier_symbol(identifier, self.scoping)
    }
}

/// Types never-written bindings in source order, lists rewritten `let`
/// bindings with a typed initializer, and the plain parameters of function
/// declarations that never mention `arguments`.
struct Collect<'t, 's> {
    types: &'t mut Types<'s>,
    rewritten: BTreeMap<SymbolId, Prim>,
    /// Function declaration symbol -> its unwritten plain parameters.
    functions: BTreeMap<SymbolId, Vec<Option<SymbolId>>>,
}

/// Each direct call of a listed function: its argument types, or `None`
/// per argument when unknown; a function used any other way, or called with
/// a spread, is `escaped`.
struct Calls<'t, 's> {
    types: &'t Types<'s>,
    functions: &'t BTreeMap<SymbolId, Vec<Option<SymbolId>>>,
    sites: BTreeMap<SymbolId, Vec<Vec<Option<Prim>>>>,
    escaped: BTreeSet<SymbolId>,
}

impl<'a> Visit<'a> for Calls<'_, '_> {
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        walk::walk_call_expression(self, call);
        self.site(call);
    }
}

impl Calls<'_, '_> {
    fn site(&mut self, call: &CallExpression<'_>) {
        let Expression::Identifier(callee) = &call.callee else {
            return;
        };
        let Some(function) = self.types.symbol(callee).filter(|symbol| self.functions.contains_key(symbol)) else {
            return;
        };
        if call.optional || call.arguments.iter().any(|argument| matches!(argument, Argument::SpreadElement(_))) {
            self.escaped.insert(function);
            return;
        }
        let types = call
            .arguments
            .iter()
            .map(|argument| argument.as_expression().and_then(|value| self.types.type_of(value)))
            .collect();
        self.sites.entry(function).or_default().push(types);
    }
}

/// `arguments` in a function's own code (arrows share it).
#[derive(Default)]
struct MentionsArguments(bool);

impl<'a> Visit<'a> for MentionsArguments {
    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        self.0 |= identifier.name == "arguments";
    }
    fn visit_function(&mut self, _: &Function<'a>, _: oxc_syntax::scope::ScopeFlags) {}
}

/// Checks each write of an assumed-typed binding: `seen` counts the writes
/// it understood, `failed` holds bindings with a write of another type.
struct CheckWrites<'t, 's> {
    types: &'t Types<'s>,
    seen: BTreeMap<SymbolId, usize>,
    failed: BTreeSet<SymbolId>,
}

impl<'a> Visit<'a> for CheckWrites<'_, '_> {
    fn visit_assignment_expression(&mut self, assignment: &AssignmentExpression<'a>) {
        walk::walk_assignment_expression(self, assignment);
        self.assignment(assignment);
    }

    fn visit_update_expression(&mut self, update: &UpdateExpression<'a>) {
        walk::walk_update_expression(self, update);
        self.update(update);
    }
}

impl CheckWrites<'_, '_> {
    fn assignment(&mut self, assignment: &AssignmentExpression<'_>) {
        let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else {
            return;
        };
        let Some(symbol) = self.types.symbol(target) else {
            return;
        };
        let Some(prim) = self.types.bindings.get(&symbol).copied() else {
            return;
        };
        *self.seen.entry(symbol).or_default() += 1;
        use oxc_syntax::operator::AssignmentOperator as Op;
        let value = self.types.type_of(&assignment.right);
        let fits = match assignment.operator {
            Op::Assign | Op::LogicalAnd | Op::LogicalOr | Op::LogicalNullish => value == Some(prim),
            // `s += v` on a string stays a string; on a number only with a
            // number-like operand.
            Op::Addition => match prim {
                Prim::String => true,
                Prim::Number => matches!(value, Some(Prim::Number | Prim::Boolean | Prim::Nullish)),
                _ => false,
            },
            // The other arithmetic operators make a number from a number
            // (a BigInt operand would throw instead).
            _ => prim == Prim::Number,
        };
        if !fits {
            self.failed.insert(symbol);
        }
    }

    fn update(&mut self, update: &UpdateExpression<'_>) {
        let SimpleAssignmentTarget::AssignmentTargetIdentifier(target) = &update.argument else {
            return;
        };
        let Some(symbol) = self.types.symbol(target) else {
            return;
        };
        if let Some(prim) = self.types.bindings.get(&symbol).copied() {
            *self.seen.entry(symbol).or_default() += 1;
            if prim != Prim::Number {
                self.failed.insert(symbol);
            }
        }
    }
}

impl<'a> Visit<'a> for Collect<'_, '_> {
    fn visit_function(&mut self, function: &Function<'a>, flags: oxc_syntax::scope::ScopeFlags) {
        let scoping = self.types.scoping;
        // A hoisted function declaration is initialized before any code of
        // its scope runs.
        if function.is_declaration() {
            if let Some(symbol) = function.id.as_ref().and_then(|id| id.symbol_id.get()) {
                if !scoping.symbol_is_mutated(symbol) {
                    self.types.declared.insert(symbol, 0);
                }
            }
        }
        // A function's first statement `let a, b;` runs before any closure
        // can: every nested read finds the bindings initialized.
        if let Some(body) = &function.body {
            if let Some(Statement::VariableDeclaration(declaration)) = body.statements.first() {
                if declaration.kind == VariableDeclarationKind::Let
                    && declaration.declarations.iter().all(|declarator| declarator.init.is_none())
                {
                    for declarator in &declaration.declarations {
                        if let BindingPattern::BindingIdentifier(binding) = &declarator.id {
                            if let Some(symbol) = binding.symbol_id.get() {
                                if scoping.symbol_declarations(symbol).count() == 1 {
                                    self.types.declared.insert(symbol, 0);
                                }
                            }
                        }
                    }
                }
            }
        }
        // Plain parameters without defaults are initialized before any of
        // the function's code runs; `arguments` could alias them.
        let plain = function.params.items.iter().all(|param| {
            matches!(param.pattern, BindingPattern::BindingIdentifier(_)) && param.initializer.is_none()
        });
        let mut mentions = MentionsArguments::default();
        if let Some(body) = &function.body {
            mentions.visit_function_body(body);
        }
        if plain {
            for param in &function.params.items {
                if let BindingPattern::BindingIdentifier(binding) = &param.pattern {
                    if let Some(symbol) = binding.symbol_id.get() {
                        self.types.declared.insert(symbol, 0);
                    }
                }
            }
        }
        if plain && !mentions.0 {
            let params: Vec<Option<SymbolId>> = function
                .params
                .items
                .iter()
                .map(|param| match &param.pattern {
                    BindingPattern::BindingIdentifier(binding) => binding
                        .symbol_id
                        .get()
                        .filter(|symbol| !scoping.symbol_is_mutated(*symbol)),
                    _ => None,
                })
                .collect();
            for symbol in params.iter().flatten() {
                self.types.declared.insert(*symbol, 0);
            }
            if function.is_declaration() {
                if let Some(symbol) = function.id.as_ref().and_then(|id| id.symbol_id.get()) {
                    if !scoping.symbol_is_mutated(symbol) && scoping.symbol_declarations(symbol).count() == 1 {
                        self.functions.insert(symbol, params);
                    }
                }
            }
        }
        walk::walk_function(self, function, flags);
    }

    fn visit_variable_declarator(&mut self, declarator: &VariableDeclarator<'a>) {
        walk::walk_variable_declarator(self, declarator);
        let (BindingPattern::BindingIdentifier(binding), Some(init)) = (&declarator.id, &declarator.init)
        else {
            return;
        };
        let Some(symbol) = binding.symbol_id.get() else {
            return;
        };
        let scoping = self.types.scoping;
        if scoping.symbol_declarations(symbol).count() != 1 {
            return;
        }
        // Any single declaration is initialized once it has run; only a
        // never-written one keeps its initializer's type.
        self.types.declared.insert(symbol, declarator.span.end);
        if scoping.symbol_is_mutated(symbol) {
            // A hoisted `var` can be read as `undefined` before its
            // initializer; a `let` cannot be read at all before it.
            if scoping.symbol_flags(symbol).contains(oxc_semantic::SymbolFlags::BlockScopedVariable) {
                if let Some(prim) = self.types.type_of(init) {
                    self.rewritten.insert(symbol, prim);
                }
            }
            return;
        }
        if let Some(prim) = self.types.type_of(init) {
            self.types.bindings.insert(symbol, prim);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Prim, Types};
    use crate::analysis;
    use oxc_allocator::Allocator;
    use oxc_ast::ast::*;

    fn last_expression<'a>(program: &'a Program<'a>) -> &'a Expression<'a> {
        let Some(Statement::ExpressionStatement(statement)) = program.body.last() else {
            panic!("ends in an expression statement");
        };
        &statement.expression
    }

    #[test]
    fn types_operator_results_and_judges_purity() {
        let allocator = Allocator::default();
        let source = "let a = Number(x) + 1; let b = typeof y; let c = a | 0; a + 2 === b;";
        let program = analysis::parse_program(&allocator, source).unwrap();
        let (scoping, nodes) = analysis::analyze(&program);
        let types = Types::build(&program, &scoping, &nodes);
        let expression = last_expression(&program);
        assert_eq!(types.type_of(expression), Some(Prim::Boolean));
        assert!(types.pure(expression));
        for impure in ["o + 1;", "a + o.p;", "f();", "a in o;", "q - 1;"] {
            let allocator = Allocator::default();
            let text = format!("let a = Number(x) + 1; {impure}");
            let program = analysis::parse_program(&allocator, &text).unwrap();
            let (scoping, nodes) = analysis::analyze(&program);
            let types = Types::build(&program, &scoping, &nodes);
            assert!(!types.pure(last_expression(&program)), "{impure}");
        }
    }

    #[test]
    fn types_a_rewritten_let_when_every_write_keeps_its_type() {
        let typed = |source: &str| {
            let allocator = Allocator::default();
            let program = analysis::parse_program(&allocator, source).unwrap();
            let (scoping, nodes) = analysis::analyze(&program);
            let types = Types::build(&program, &scoping, &nodes);
            types.type_of(last_expression(&program))
        };
        assert_eq!(typed("let i = 0; let j = 1; i = j + 1; j = i << 2; i++; j -= 3; i;"), Some(Prim::Number));
        assert_eq!(typed("let s = \"\"; s += o; s;"), Some(Prim::String));
        // one write of another type, or one this scan cannot type
        assert_eq!(typed("let i = 0; i = o; i;"), None);
        assert_eq!(typed("let i = 0; [i] = o; i;"), None);
        assert_eq!(typed("let i = 0; for (i in o); i;"), None);
        assert_eq!(typed("let i = 0; i += o; i;"), None);
        // a var can be read before its initializer runs
        assert_eq!(typed("var i = 0; i = 1; i;"), None);
    }

    #[test]
    fn types_a_parameter_by_every_direct_call() {
        let typed = |source: &str| {
            let allocator = Allocator::default();
            let program = analysis::parse_program(&allocator, source).unwrap();
            let (scoping, nodes) = analysis::analyze(&program);
            let types = Types::build(&program, &scoping, &nodes);
            let Some(Statement::FunctionDeclaration(function)) = program.body.first() else {
                panic!("starts with the function");
            };
            let Some(Statement::ReturnStatement(ret)) = function.body.as_ref().unwrap().statements.last() else {
                panic!("ends in a return");
            };
            (types.type_of(ret.argument.as_ref().unwrap()), types.pure(ret.argument.as_ref().unwrap()))
        };
        assert_eq!(typed("function h(a, b) { return a >> 16; }\nh(1, 2); h(3 | x);"), (Some(Prim::Number), true));
        // a recursive helper passing its own typed values
        assert_eq!(typed("function h(a) { return h(a - 1) >> 16; }\nh(1);").0, Some(Prim::Number));
    }
}
