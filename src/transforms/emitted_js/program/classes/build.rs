//! Writing the `class`: its elements, the subclass link and `super` calls.

use super::*;

impl<'s, 'a> Scan<'s, 'a> {
    pub(super) fn list(&mut self, statements: &[Statement<'a>], container: Span) {
        for (index, statement) in statements.iter().enumerate() {
            if let Some(constructor) = thread(statement) {
                if self.babel_constructor(constructor).is_some() {
                    self.babel_class(statements, constructor, container);
                } else {
                    self.class(statements, index, constructor, container);
                }
            }
        }
    }

    pub(super) fn class(&mut self, statements: &[Statement<'a>], index: usize, constructor: &Function<'a>, container: Span) -> Option<()> {
        if !strict(constructor) {
            return None;
        }
        let class = constructor.id.as_ref()?.symbol_id();
        let Some(holders) = self.holders(class) else {
            // Refused: a use a class does not allow.
            return None;
        };
        // TypeScript's subclass constructor: `return S.apply(this, arguments) || this;`.
        let base = self.super_constructor(constructor);
        if let Some(base) = base {
            if !self.base_set_before(base, constructor) {
                // Refused: the superclass is not set before the class.
                return None;
            }
        }
        let (linked, members, consumed) = self.class_members(statements, index, &holders, base);
        if members.is_empty() && base.is_none() {
            // Refused: no members.
            return None;
        }
        if base.is_some() && linked != [true; 3] {
            // Refused: a subclass link is missing.
            return None;
        }
        let (consumed, consumed_spans) = self.consumed_with_copies(statements, index, &members, consumed, base)?;
        if !self.used_after_class(class, &consumed_spans, constructor, container) {
            return None;
        }
        let text = self.class_text(statements, constructor, base, &members)?;
        self.edits.push(analysis::Edit::replace(constructor.span, text));
        for member in &members {
            self.edits.push(analysis::Edit::delete(whole_line(self.source, statements[member.function].span())));
        }
        for &at in &consumed {
            self.edits.push(analysis::Edit::delete(whole_line(self.source, statements[at].span())));
        }
        self.count += 1;
        Some(())
    }

    /// `S` itself holds the base from before the class on.
    fn base_set_before(&self, base: SymbolId, constructor: &Function<'a>) -> bool {
        let mut writes = self.scoping.get_resolved_references(base).filter(|reference| reference.is_write());
        let declared = self.scoping.symbol_span(base).end <= constructor.span.start;
        let written_before = writes.all(|reference| self.span(reference.node_id()).end <= constructor.span.start);
        declared && written_before
    }

    /// The statements after the constructor that build the class: which
    /// subclass links were seen, the members, and the consumed statements.
    fn class_members(&self, statements: &[Statement<'a>], index: usize, holders: &[SymbolId], base: Option<SymbolId>) -> ([bool; 3], Vec<Member>, Vec<usize>) {
        let mut linked = [false; 3];
        // Members: stores of single-use strict threads into the class or a
        // copy of it, and inlined `_createClass` descriptor blocks, up to
        // anything that could see the class half built.
        let mut members: Vec<Member> = Vec::new();
        let mut consumed: Vec<usize> = Vec::new();
        let mut at = index + 1;
        while at < statements.len() {
            let statement = &statements[at];
            if thread(statement).is_some() {
                at += 1;
                continue;
            }
            if let Some(base) = base {
                if let Some(link) = self.subclass_link(statement, holders, base) {
                    linked[link] = true;
                    consumed.push(at);
                    at += 1;
                    continue;
                }
            }
            if let Some(store) = self.store(statement) {
                if holders.contains(&store.holder) {
                    let reserved = matches!(store.key.as_str(), "constructor" | "prototype") || !store.instance && matches!(store.key.as_str(), "name" | "length");
                    match self.method(statements, index + 1, at, store.value) {
                        Some(function) if !reserved => {
                            members.push(Member { function, key: store.key, instance: store.instance, kind: "" });
                            consumed.push(at);
                            at += 1;
                            continue;
                        }
                        _ => {
                            break;
                        }
                    }
                }
            }
            if let Some((last, block)) = self.descriptors(statements, index, at, holders) {
                members.extend(block);
                consumed.extend(at..=last);
                at = last + 1;
                continue;
            }
            // A copy of the class into another variable sees nothing.
            if self.copy(statement).is_some_and(|value| holders.contains(&value)) {
                at += 1;
                continue;
            }
            let mut runs = Runs::default();
            runs.visit_statement(statement);
            let mentions = holders.iter().any(|&holder| self.references(holder).iter().any(|&node| {
                let span = self.span(node);
                statement.span().start <= span.start && span.end <= statement.span().end
            }));
            if runs.found || mentions {
                break;
            }
            at += 1;
        }
        (linked, members, consumed)
    }

    /// The statements to delete besides the moved functions, with their
    /// spans: the consumed ones, plus copies that only they read.
    fn consumed_with_copies(
        &self,
        statements: &[Statement<'a>],
        index: usize,
        members: &[Member],
        consumed: Vec<usize>,
        base: Option<SymbolId>,
    ) -> Option<(Vec<usize>, Vec<Span>)> {
        // Moved functions are not consumed statements: their declarations go.
        let mut consumed: Vec<usize> = consumed.into_iter().filter(|at| !members.iter().any(|member| member.function == *at)).collect();
        let mut consumed_spans: Vec<Span> = consumed.iter().map(|&at| statements[at].span()).collect();
        // A copy only the consumed links (or the super calls) read goes too.
        let mut gone: Vec<Span> = consumed_spans.clone();
        if let Some(base) = base {
            for member in members.iter().filter(|member| member.instance) {
                gone.extend(self.super_calls(thread(&statements[member.function])?, base).into_iter().map(|(span, _)| span));
            }
        }
        let last = consumed.iter().chain(members.iter().map(|member| &member.function)).copied().max().unwrap_or(index);
        for (at, statement) in statements.iter().enumerate().take(last).skip(index + 1) {
            let Statement::VariableDeclaration(declaration) = statement else { continue };
            let [declarator] = declaration.declarations.as_slice() else { continue };
            let BindingPattern::BindingIdentifier(id) = &declarator.id else { continue };
            if declarator.init.as_ref().and_then(|init| self.symbol(init)).is_none() {
                continue;
            }
            let dead = self.references(id.symbol_id()).iter().all(|&node| {
                let span = self.span(node);
                gone.iter().any(|outer| outer.contains_inclusive(span))
            });
            if dead && base.is_some() {
                consumed.push(at);
                consumed_spans.push(statement.span());
            }
        }
        Some((consumed, consumed_spans))
    }

    /// Every other use of the class's own name: inside the list, after
    /// the class. (A copy holds the class only once the copy ran, as
    /// before.)
    fn used_after_class(&self, class: SymbolId, consumed_spans: &[Span], constructor: &Function<'a>, container: Span) -> bool {
        for reference in self.scoping.get_resolved_references(class) {
            let node = reference.node_id();
            let span = self.span(node);
            if consumed_spans.iter().any(|store| store.start <= span.start && span.end <= store.end) {
                continue;
            }
            if span.start < container.start || span.end > container.end || !self.runs_after(node, constructor.span.end, 3) {
                // Refused: used before the class or outside its list.
                return false;
            }
        }
        true
    }

    /// The class text, at the constructor's indentation.
    fn class_text(&self, statements: &[Statement<'a>], constructor: &Function<'a>, base: Option<SymbolId>, members: &[Member]) -> Option<String> {
        let line_start = self.source[..constructor.span.start as usize].rfind('\n').map_or(0, |at| at + 1);
        let indent = &self.source[line_start..constructor.span.start as usize];
        if !indent.trim().is_empty() {
            return None;
        }
        let name = constructor.id.as_ref()?.name.as_str();
        let mut text = match base {
            Some(base) => {
                let mut text = format!("class {name} extends {} {{\n", self.scoping.symbol_name(base));
                text.push_str(&format!("{indent}\tconstructor() {{\n{indent}\t\tsuper(...arguments);\n{indent}\t}}\n"));
                text
            }
            None => {
                let mut text = format!("class {name} {{\n");
                text.push_str(&self.element(indent, "constructor", constructor, &[])?);
                text
            }
        };
        for member in members {
            let function = thread(&statements[member.function])?;
            let key = if analysis::is_identifier_name(&member.key) { member.key.clone() } else { format!("[{:?}]", member.key) };
            let head = format!("{}{}{key}", if member.instance { "" } else { "static " }, member.kind);
            let supers = match base {
                Some(base) if member.instance => self.super_calls(function, base),
                _ => Vec::new(),
            };
            text.push_str(&self.element(indent, &head, function, &supers)?);
        }
        text.push_str(indent);
        text.push('}');
        Some(text)
    }

    /// `head(params) { body }`, one level deeper than `indent`, without the
    /// `"use strict"` directive the class body implies, with `replace`d
    /// spans of the body rewritten.
    pub(super) fn element(&self, indent: &str, head: &str, function: &Function<'a>, replace: &[(Span, String)]) -> Option<String> {
        self.element_lines(indent, head, function, replace, "\t")
    }

    /// [`Self::element`], prefixing each body line with `shift`.
    pub(super) fn element_lines(&self, indent: &str, head: &str, function: &Function<'a>, replace: &[(Span, String)], shift: &str) -> Option<String> {
        let body = function.body.as_ref()?;
        let params = text(self.source, function.params.span);
        let params = params.strip_prefix('(').and_then(|p| p.strip_suffix(')')).unwrap_or(params);
        let mut out = format!("{indent}\t{head}({params}) {{\n");
        let mut edits: Vec<(Span, String)> = body.directives.iter().filter(|d| d.directive == "use strict").map(|d| (d.span, String::new())).collect();
        edits.extend(replace.iter().cloned());
        edits.sort_by_key(|(span, _)| span.start);
        let mut at = body.span.start as usize + 1;
        let mut inner = String::new();
        for (span, replacement) in edits {
            if (span.start as usize) < at {
                return None;
            }
            inner.push_str(&self.source[at..span.start as usize]);
            inner.push_str(&replacement);
            at = span.end as usize;
        }
        inner.push_str(&self.source[at..body.span.end as usize - 1]);
        for line in inner.lines() {
            if line.trim().is_empty() {
                continue;
            }
            out.push_str(shift);
            out.push_str(line);
            out.push('\n');
        }
        out.push_str(&format!("{indent}\t}}\n"));
        Some(out)
    }

    /// Follows single-store copies (`let a = b;`) back to their source.
    pub(super) fn origin(&self, mut symbol: SymbolId) -> SymbolId {
        for _ in 0..8 {
            let writes: Vec<_> = self.scoping.get_resolved_references(symbol).filter(|reference| reference.is_write()).map(|reference| reference.node_id()).collect();
            let declarator = self.nodes.kind(self.scoping.symbol_declaration(symbol));
            let value = match (writes.as_slice(), declarator) {
                ([], AstKind::VariableDeclarator(declarator)) => declarator.init.as_ref().and_then(|init| self.symbol(init)),
                ([write], AstKind::VariableDeclarator(declarator)) if declarator.init.is_none() => match self.parent(*write).1 {
                    AstKind::AssignmentExpression(assignment) => self.symbol(&assignment.right),
                    _ => None,
                },
                _ => None,
            };
            match value {
                Some(value) => symbol = value,
                None => break,
            }
        }
        symbol
    }

    /// `return S["apply"](this, arguments) || this;` as the whole body: `S`.
    pub(super) fn super_constructor(&self, constructor: &Function<'a>) -> Option<SymbolId> {
        let body = constructor.body.as_ref()?;
        let [Statement::ReturnStatement(returned)] = body.statements.as_slice() else {
            return None;
        };
        let Expression::LogicalExpression(either) = returned.argument.as_ref()?.get_inner_expression() else {
            return None;
        };
        if either.operator != oxc_syntax::operator::LogicalOperator::Or || !matches!(either.right.get_inner_expression(), Expression::ThisExpression(_)) {
            return None;
        }
        let Expression::CallExpression(call) = either.left.get_inner_expression() else {
            return None;
        };
        let (object, key) = member_key(call.callee.get_inner_expression().as_member_expression()?)?;
        let arguments_ok = matches!(call.arguments.as_slice(), [Argument::ThisExpression(_), Argument::Identifier(arguments)] if arguments.name == "arguments");
        (key == "apply" && arguments_ok).then_some(self.symbol(object)?)
    }

    /// One of TypeScript's three subclass links: `C.prototype =
    /// Object.create(S.prototype)` (0), `C.prototype.constructor = C` (1),
    /// `extendStatics(C, S)` (2).
    pub(super) fn subclass_link(&self, statement: &Statement<'a>, holders: &[SymbolId], base: SymbolId) -> Option<usize> {
        let Statement::ExpressionStatement(expression) = statement else {
            return None;
        };
        let base = self.origin(base);
        let is_base = |expression: &Expression<'_>| self.symbol(expression).is_some_and(|symbol| self.origin(symbol) == base);
        let is_holder = |expression: &Expression<'_>| self.symbol(expression).is_some_and(|symbol| holders.contains(&symbol));
        match &expression.expression {
            Expression::AssignmentExpression(assignment) if assignment.operator == oxc_syntax::operator::AssignmentOperator::Assign => {
                let (object, key) = member_key(assignment.left.as_member_expression()?)?;
                if key == "prototype" && is_holder(object) {
                    let Expression::CallExpression(create) = assignment.right.get_inner_expression() else {
                        return None;
                    };
                    let [Argument::ComputedMemberExpression(_) | Argument::StaticMemberExpression(_)] = create.arguments.as_slice() else {
                        return None;
                    };
                    let (prototype_of, prototype) = member_key(create.arguments[0].as_expression()?.as_member_expression()?)?;
                    return (text(self.source, create.callee.span()) == "Object[\"create\"]" && prototype == "prototype" && is_base(prototype_of)).then_some(0);
                }
                let (prototype_of, prototype) = member_key(object.get_inner_expression().as_member_expression()?)?;
                (key == "constructor" && prototype == "prototype" && is_holder(prototype_of) && is_holder(&assignment.right)).then_some(1)
            }
            Expression::CallExpression(call) if self.extend_statics(&call.callee) => {
                let [Argument::Identifier(_), Argument::Identifier(_)] = call.arguments.as_slice() else {
                    return None;
                };
                (is_holder(call.arguments[0].as_expression()?) && is_base(call.arguments[1].as_expression()?)).then_some(2)
            }
            _ => None,
        }
    }

    /// `S["prototype"]["m"]["call"](this, …)` in a method (not in a nested
    /// function): `super.m(…)`.
    pub(super) fn super_calls(&self, function: &Function<'a>, base: SymbolId) -> Vec<(Span, String)> {
        let base = self.origin(base);
        let mut out = Vec::new();
        for node in self.nodes.iter() {
            let AstKind::CallExpression(call) = node.kind() else {
                continue;
            };
            if !function.span.contains_inclusive(call.span) {
                continue;
            }
            let own = self.nodes.ancestor_ids(node.id()).find_map(|ancestor| match self.nodes.kind(ancestor) {
                AstKind::Function(inner) => Some(inner.span == function.span),
                _ => None,
            });
            if own != Some(true) {
                continue;
            }
            let Some((method, call_key)) = call.callee.get_inner_expression().as_member_expression().and_then(member_key) else {
                continue;
            };
            let Some((prototype, key)) = method.get_inner_expression().as_member_expression().and_then(member_key) else {
                continue;
            };
            let Some((holder, prototype_key)) = prototype.get_inner_expression().as_member_expression().and_then(member_key) else {
                continue;
            };
            if call_key != "call" || prototype_key != "prototype" || self.symbol(holder).is_none_or(|symbol| self.origin(symbol) != base) {
                continue;
            }
            let Some(Argument::ThisExpression(_)) = call.arguments.first() else {
                continue;
            };
            let rest: Vec<&str> = call.arguments.iter().skip(1).map(|argument| text(self.source, argument.span())).collect();
            let method = if analysis::is_identifier_name(&key) { format!("super.{key}") } else { format!("super[{key:?}]") };
            out.push((call.span, format!("{method}({})", rest.join(", "))));
        }
        out
    }
}
