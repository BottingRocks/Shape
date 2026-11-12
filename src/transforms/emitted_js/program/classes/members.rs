//! A class's parts: method stores, descriptors, holders and the reads that must run after.

use super::*;

impl<'s, 'a> Scan<'s, 'a> {
    pub(super) fn symbol(&self, expression: &Expression<'_>) -> Option<SymbolId> {
        match expression.get_inner_expression() {
            Expression::Identifier(identifier) => self.scoping.get_reference(identifier.reference_id()).symbol_id(),
            _ => None,
        }
    }

    pub(super) fn store(&self, statement: &Statement<'a>) -> Option<Store> {
        let Statement::ExpressionStatement(expression) = statement else {
            return None;
        };
        let Expression::AssignmentExpression(assignment) = &expression.expression else {
            return None;
        };
        if assignment.operator != oxc_syntax::operator::AssignmentOperator::Assign {
            return None;
        }
        let (object, key) = member_key(assignment.left.as_member_expression()?)?;
        let value = self.symbol(&assignment.right)?;
        if let Some(prototype) = object.get_inner_expression().as_member_expression() {
            let (holder, name) = member_key(prototype)?;
            return (name == "prototype").then_some(Store { holder: self.symbol(holder)?, instance: true, key, value });
        }
        Some(Store { holder: self.symbol(object)?, instance: false, key, value })
    }

    /// Strict, or sloppy in no way strict mode would change.
    pub(super) fn strict_safe(&self, function: &Function<'a>) -> bool {
        if strict(function) {
            return true;
        }
        let mut sloppy = SloppyOnly::default();
        sloppy.visit_function(function, oxc_syntax::scope::ScopeFlags::Function);
        sloppy.clean() && sloppy.unresolved_writes.iter().all(|&reference| self.scoping.get_reference(reference).symbol_id().is_some())
    }

    /// The single-use thread `value` declared in `from..to`, strict or
    /// safe to make strict.
    pub(super) fn method(&self, statements: &[Statement<'a>], from: usize, to: usize, value: SymbolId) -> Option<usize> {
        let offset = statements[from..to].iter().position(|candidate| {
            thread(candidate).is_some_and(|function| function.id.as_ref().is_some_and(|id| id.symbol_id() == value))
        })?;
        let single = self.scoping.get_resolved_references(value).count() == 1;
        (single && thread(&statements[from + offset]).is_some_and(|function| self.strict_safe(function))).then_some(from + offset)
    }


    /// Babel's `_createClass(C, [{ key, get }])` inlined:
    ///
    /// ```js
    /// let p = { ["key"]: "k" };
    /// function G() { "use strict"; … }
    /// p["get"] = G;
    /// p = [p];
    /// for (let i = 0; i < p["length"]; i++) { …defineProperty(C["prototype"], …, p[i]); }
    /// Object["defineProperty"](C, "prototype", { ["writable"]: false });
    /// ```
    ///
    /// The loop marks each property non-enumerable and configurable (and a
    /// value writable), as class elements are. Returns the block's last
    /// statement and its elements.
    pub(super) fn descriptors(&self, statements: &[Statement<'a>], class_at: usize, at: usize, holders: &[SymbolId]) -> Option<(usize, Vec<Member>)> {
        let Statement::VariableDeclaration(declaration) = &statements[at] else {
            return None;
        };
        let [declarator] = declaration.declarations.as_slice() else {
            return None;
        };
        let BindingPattern::BindingIdentifier(descriptor) = &declarator.id else {
            return None;
        };
        let descriptor = descriptor.symbol_id();
        let Some(Expression::ObjectExpression(object)) = declarator.init.as_ref().map(|init| init.get_inner_expression()) else {
            return None;
        };
        let [ObjectPropertyKind::ObjectProperty(property)] = object.properties.as_slice() else {
            return None;
        };
        let key_name = match &property.key {
            PropertyKey::StaticIdentifier(name) => name.name.as_str(),
            PropertyKey::StringLiteral(name) => name.value.as_str(),
            _ => return None,
        };
        let Expression::StringLiteral(key) = property.value.get_inner_expression() else {
            return None;
        };
        if key_name != "key" {
            return None;
        }
        // Accessor and value stores, up to `p = [p];`.
        let mut fields: Vec<(&'static str, SymbolId)> = Vec::new();
        let mut cursor = at + 1;
        loop {
            let statement = statements.get(cursor)?;
            cursor += 1;
            if thread(statement).is_some() {
                continue;
            }
            if let Some(store) = self.store(statement) {
                if store.holder == descriptor && !store.instance {
                    let kind = match store.key.as_str() {
                        "get" => "get ",
                        "set" => "set ",
                        "value" => "",
                        _ => return None,
                    };
                    fields.push((kind, store.value));
                    continue;
                }
                return None;
            }
            // p = [p];
            let Statement::ExpressionStatement(expression) = statement else {
                return None;
            };
            let Expression::AssignmentExpression(assignment) = &expression.expression else {
                return None;
            };
            let Expression::ArrayExpression(array) = assignment.right.get_inner_expression() else {
                return None;
            };
            let single = matches!(array.elements.as_slice(), [element] if element.as_expression().and_then(|e| self.symbol(e)) == Some(descriptor));
            let target = match &assignment.left {
                AssignmentTarget::AssignmentTargetIdentifier(target) => self.scoping.get_reference(target.reference_id()).symbol_id(),
                _ => None,
            };
            if !single || target != Some(descriptor) {
                return None;
            }
            break;
        }
        // The defining loop.
        let Statement::ForStatement(define) = statements.get(cursor)? else {
            return None;
        };
        let loop_text = text(self.source, define.span);
        let (target, instance) = holders.iter().find_map(|&holder| {
            let name = self.scoping.symbol_name(holder);
            if loop_text.contains(&format!("Object[\"defineProperty\"]({name}[\"prototype\"], ")) {
                Some((holder, true))
            } else if loop_text.contains(&format!("Object[\"defineProperty\"]({name}, ")) {
                Some((holder, false))
            } else {
                None
            }
        })?;
        let _ = target;
        if !["[\"enumerable\"] = ", "[\"configurable\"] = true", "@@toPrimitive must return a primitive value."].iter().all(|needle| loop_text.contains(needle)) {
            return None;
        }
        let mut last = cursor;
        // `p` lives only in the block.
        let block = Span::new(statements[at].span().start, define.span.end);
        if self.references(descriptor).iter().any(|&node| {
            let span = self.span(node);
            span.start < block.start || span.end > block.end
        }) {
            return None;
        }
        // Object["defineProperty"](C, "prototype", { ["writable"]: false });
        if let Some(Statement::ExpressionStatement(sealed)) = statements.get(cursor + 1) {
            let sealed_text = text(self.source, sealed.span);
            if holders.iter().any(|&holder| sealed_text == format!("Object[\"defineProperty\"]({}, \"prototype\", {{ [\"writable\"]: false }});", self.scoping.symbol_name(holder))) {
                last = cursor + 1;
            }
        }
        let mut members = Vec::new();
        for (kind, value) in fields {
            let function = self.method(statements, class_at + 1, cursor, value)?;
            members.push(Member { function, key: key.value.to_string(), instance, kind });
        }
        Some((last, members))
    }

    /// `let a = b;` / `a = b;`: `b`.
    pub(super) fn copy(&self, statement: &Statement<'a>) -> Option<SymbolId> {
        match statement {
            Statement::VariableDeclaration(declaration) => match declaration.declarations.as_slice() {
                [declarator] if matches!(declarator.id, BindingPattern::BindingIdentifier(_)) => self.symbol(declarator.init.as_ref()?),
                _ => None,
            },
            Statement::ExpressionStatement(expression) => match &expression.expression {
                Expression::AssignmentExpression(assignment)
                    if assignment.operator == oxc_syntax::operator::AssignmentOperator::Assign
                        && matches!(assignment.left, AssignmentTarget::AssignmentTargetIdentifier(_)) =>
                {
                    self.symbol(&assignment.right)
                }
                _ => None,
            },
            _ => None,
        }
    }

    pub(super) fn references(&self, symbol: SymbolId) -> Vec<NodeId> {
        self.scoping.get_resolved_references(symbol).map(|reference| reference.node_id()).collect()
    }

    pub(super) fn span(&self, node: NodeId) -> Span {
        self.nodes.get_node(node).span()
    }

    /// The parent of a reference, past parentheses.
    pub(super) fn parent(&self, node: NodeId) -> (NodeId, AstKind<'a>) {
        let mut at = node;
        loop {
            let parent = self.nodes.parent_id(at);
            match self.nodes.kind(parent) {
                AstKind::ParenthesizedExpression(_) => at = parent,
                kind => return (parent, kind),
            }
        }
    }

    /// Every variable the class can reach, from `symbol`, when each use is
    /// one a class allows; `None` when one is not. `returns` collects the
    /// functions it is returned from.
    pub(super) fn holders(&self, class: SymbolId) -> Option<Vec<SymbolId>> {
        self.holders_except(class, &[])
    }

    /// [`Self::holders`], not counting uses inside `ignore` (code the
    /// rewrite removes).
    pub(super) fn holders_except(&self, class: SymbolId, ignore: &[Span]) -> Option<Vec<SymbolId>> {
        let mut holders = vec![class];
        let mut seen: HashSet<SymbolId> = HashSet::from([class]);
        let mut work = vec![class];
        // Expressions holding the class: a variable's reads, or an
        // `arguments[n]` it was passed as.
        let mut uses: Vec<(NodeId, Span, SymbolId)> = Vec::new();
        let mut add = |symbol: SymbolId, work: &mut Vec<SymbolId>, holders: &mut Vec<SymbolId>| {
            if seen.insert(symbol) {
                holders.push(symbol);
                work.push(symbol);
            }
        };
        loop {
            while let Some(holder) = work.pop() {
                for reference in self.scoping.get_resolved_references(holder) {
                    if !reference.is_write() {
                        uses.push((reference.node_id(), self.span(reference.node_id()), holder));
                    }
                }
            }
            let Some((node, span, holder)) = uses.pop() else {
                break;
            };
            if ignore.iter().any(|outer| outer.contains_inclusive(span)) {
                continue;
            }
            let (parent, kind) = self.parent(node);
            match kind {
                AstKind::NewExpression(new) if new.callee.get_inner_expression().span() == span => {}
                AstKind::BinaryExpression(binary)
                    if binary.operator == oxc_syntax::operator::BinaryOperator::Instanceof
                        && binary.right.get_inner_expression().span() == span => {}
                AstKind::Class(heritage) if heritage.super_class.as_ref().is_some_and(|base| base.get_inner_expression().span() == span) => {}
                // Object["defineProperty"](C, "prototype", { ["writable"]: false }): a
                // class's prototype already is.
                AstKind::CallExpression(call)
                    if text(self.source, call.span).starts_with("Object[\"defineProperty\"](")
                        && text(self.source, call.span).ends_with(", \"prototype\", { [\"writable\"]: false })")
                        && call.arguments.first().is_some_and(|argument| argument.span() == span) => {}
                // extendStatics(C, S): the subclass link.
                AstKind::CallExpression(call)
                    if self.extend_statics(&call.callee)
                        && call.arguments.len() == 2
                        && call.arguments.iter().any(|argument| argument.span() == span) => {}
                // Handed to a thread: its `arguments[n]` reads hold it.
                AstKind::CallExpression(call) if call.callee.get_inner_expression().span() != span => {
                    let index = call.arguments.iter().position(|argument| argument.span() == span)?;
                    if call.arguments[..index].iter().any(|argument| matches!(argument, Argument::SpreadElement(_))) {
                        return None;
                    }
                    let callee = self.symbol(&call.callee)?;
                    uses.extend(self.argument_reads(callee, index)?.into_iter().map(|(node, span)| (node, span, holder)));
                }
                AstKind::StaticMemberExpression(member) if member.object.get_inner_expression().span() == span => {
                    // Calling the function itself: a class cannot be.
                    if matches!(member.property.name.as_str(), "apply" | "call" | "bind") {
                        return self.refuse(holder, span, "apply/call/bind");
                    }
                }
                AstKind::ComputedMemberExpression(member) if member.object.get_inner_expression().span() == span => {
                    if matches!(&member.expression, Expression::StringLiteral(key) if matches!(key.value.as_str(), "apply" | "call" | "bind")) {
                        return self.refuse(holder, span, "apply/call/bind");
                    }
                    if !matches!(member.expression, Expression::StringLiteral(_) | Expression::NumericLiteral(_)) {
                        return self.refuse(holder, span, "computed member");
                    }
                }
                AstKind::VariableDeclarator(declarator) => {
                    let BindingPattern::BindingIdentifier(id) = &declarator.id else {
                        return None;
                    };
                    add(id.symbol_id(), &mut work, &mut holders);
                }
                AstKind::AssignmentExpression(assignment) if assignment.right.get_inner_expression().span() == span => {
                    // C.prototype.constructor = C, a subclass link.
                    let constructor_link = assignment.left.as_member_expression().and_then(member_key).is_some_and(|(object, key)| {
                        key == "constructor"
                            && object.get_inner_expression().as_member_expression().and_then(member_key).is_some_and(|(of, prototype)| {
                                prototype == "prototype" && self.symbol(of) == Some(holder)
                            })
                    });
                    if constructor_link {
                        continue;
                    }
                    let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else {
                        return self.refuse(holder, span, "stored on an object");
                    };
                    add(self.scoping.get_reference(target.reference_id()).symbol_id()?, &mut work, &mut holders);
                }
                AstKind::ReturnStatement(_) => {
                    // A factory: each call's result holds the class.
                    let function = self.nodes.ancestor_ids(parent).find_map(|ancestor| match self.nodes.kind(ancestor) {
                        AstKind::Function(function) => Some(function),
                        _ => None,
                    })?;
                    let factory = function.id.as_ref()?.symbol_id();
                    for call in self.references(factory) {
                        let call_span = self.span(call);
                        let (call_node, AstKind::CallExpression(called)) = self.parent(call) else {
                            return None;
                        };
                        if called.callee.get_inner_expression().span() != call_span {
                            return None;
                        }
                        match self.parent(call_node).1 {
                            AstKind::VariableDeclarator(declarator) => {
                                let BindingPattern::BindingIdentifier(id) = &declarator.id else {
                                    return None;
                                };
                                add(id.symbol_id(), &mut work, &mut holders);
                            }
                            AstKind::AssignmentExpression(assignment) => {
                                let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else {
                                    return None;
                                };
                                add(self.scoping.get_reference(target.reference_id()).symbol_id()?, &mut work, &mut holders);
                            }
                            // new (F())(…)
                            AstKind::NewExpression(new) if new.callee.get_inner_expression().span() == called.span => {}
                            // F()["encode"]: a static member read.
                            AstKind::StaticMemberExpression(member) if !matches!(member.property.name.as_str(), "apply" | "call" | "bind") => {}
                            AstKind::ComputedMemberExpression(member)
                                if matches!(&member.expression, Expression::StringLiteral(key) if !matches!(key.value.as_str(), "apply" | "call" | "bind")) => {}
                            kind => return self.refuse(holder, call_span, &format!("factory result in {}", kind.debug_name())),
                        }
                    }
                }
                kind => return self.refuse(holder, span, kind.debug_name().as_ref()),
            }
        }
        Some(holders)
    }

    /// Refuse a use of `holder`; `_why` documents the reason at the call site.
    pub(super) fn refuse<T>(&self, _holder: SymbolId, _span: Span, _why: &str) -> Option<T> {
        None
    }

    /// The `arguments[index]` reads of the thread `callee`, when it reads
    /// its arguments only by literal index.
    pub(super) fn argument_reads(&self, callee: SymbolId, index: usize) -> Option<Vec<(NodeId, Span)>> {
        let declaration = self.scoping.symbol_declaration(callee);
        let AstKind::Function(function) = self.nodes.kind(declaration) else {
            return None;
        };
        if !function.params.items.is_empty() || function.params.rest.is_some() {
            return None;
        }
        let mut reads = Vec::new();
        for node in self.nodes.iter() {
            let AstKind::IdentifierReference(identifier) = node.kind() else {
                continue;
            };
            if identifier.name != "arguments" || !function.span.contains_inclusive(identifier.span) {
                continue;
            }
            // Its own `arguments`, not a nested function's.
            let own = self.nodes.ancestor_ids(node.id()).find_map(|ancestor| match self.nodes.kind(ancestor) {
                AstKind::Function(inner) => Some(inner.span == function.span),
                _ => None,
            })?;
            if !own {
                continue;
            }
            let (member_node, AstKind::ComputedMemberExpression(member)) = self.parent(node.id()) else {
                return None;
            };
            let Expression::NumericLiteral(at) = &member.expression else {
                return None;
            };
            if at.value == index as f64 {
                reads.push((member_node, member.span));
            }
        }
        Some(reads)
    }

    /// TypeScript's `extendStatics`: every value stored in the callee is a
    /// thread that sets the prototype of its first argument to its second
    /// (`Object.setPrototypeOf`, or `__proto__`), or one it stores itself.
    pub(super) fn extend_statics(&self, callee: &Expression<'_>) -> bool {
        let Some(symbol) = self.symbol(callee) else {
            return false;
        };
        let helpers: Vec<Span> = self
            .scoping
            .get_resolved_references(symbol)
            .filter(|reference| reference.is_write())
            .filter_map(|reference| match self.parent(reference.node_id()).1 {
                AstKind::AssignmentExpression(assignment) => self.symbol(&assignment.right),
                _ => None,
            })
            .filter_map(|value| match self.nodes.kind(self.scoping.symbol_declaration(value)) {
                AstKind::Function(function) => {
                    let body = text(self.source, function.span);
                    (body.contains("Object[\"setPrototypeOf\"]") && body.contains("[\"__proto__\"] = arguments[1]")).then_some(function.span)
                }
                _ => None,
            })
            .collect();
        !helpers.is_empty()
            && self.scoping.get_resolved_references(symbol).filter(|reference| reference.is_write()).all(|reference| {
                let span = self.span(reference.node_id());
                helpers.iter().any(|helper| helper.contains_inclusive(span))
                    || matches!(self.parent(reference.node_id()).1, AstKind::AssignmentExpression(assignment) if self.symbol(&assignment.right).is_some_and(|value| helpers.contains(&self.nodes.kind(self.scoping.symbol_declaration(value)).span())))
            })
    }

    /// `node` can only run after `at`: it sits after it, and so does every
    /// function around it (and every use of such a function, a few levels
    /// out).
    pub(super) fn runs_after(&self, node: NodeId, at: u32, depth: usize) -> bool {
        if self.span(node).start < at {
            return false;
        }
        for ancestor in self.nodes.ancestor_ids(node) {
            match self.nodes.kind(ancestor) {
                AstKind::Function(function) => {
                    if function.span.start < at {
                        // Inside the class's own constructor or an enclosing
                        // function: it runs when that does.
                        return true;
                    }
                    if function.is_declaration() {
                        let Some(id) = &function.id else { return false };
                        return depth > 0 && self.references(id.symbol_id()).into_iter().all(|use_| self.runs_after(use_, at, depth - 1));
                    }
                    return true;
                }
                AstKind::ArrowFunctionExpression(arrow) if arrow.span.start >= at => return true,
                _ => {}
            }
        }
        true
    }
}
