//! Collect construction-only data descriptors into object and array literals. Values
//! remain at their original evaluation points until later SSA reconstruction.
use super::*;

pub(crate) struct FoldedLiterals {
    pub code: String,
    pub constructions: usize,
    pub arrays: usize,
    pub writes: usize,
}

pub(crate) fn fold_descriptor_literals(
    source: &str,
    protected: &BTreeSet<String>,
) -> Result<FoldedLiterals, String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut scan = Objects {
        source,
        protected,
        scoping: &scoping,
        nodes: &nodes,
        next_field: 0,
        edits: Vec::new(),
        dynamic: false,
        arrays: BTreeSet::new(),
    };
    scan.visit_program(&program);
    if scan.dynamic {
        return Ok(FoldedLiterals {
            code: source.into(),
            constructions: 0,
            arrays: 0,
            writes: 0,
        });
    }
    // An outer reconstruction preserves its RHS source verbatim. Nested
    // constructions can be revisited in the next cleanup round.
    scan.edits
        .sort_by_key(|e| (e.start, std::cmp::Reverse(e.end)));
    let mut end = 0;
    scan.edits.retain(|e| {
        if e.start < end {
            return false;
        }
        end = e.end;
        true
    });
    let count = scan
        .edits
        .iter()
        .filter(|e| e.replacement.is_empty())
        .count();
    let writes = scan.edits.len() - count;
    let arrays = scan
        .edits
        .iter()
        .filter(|e| e.replacement.is_empty() && scan.arrays.contains(&e.start))
        .count();
    let applied = analysis::apply_edits(source, scan.edits);
    if !applied.rejected.is_empty() {
        return Err("object literal edits overlap".into());
    }
    Ok(FoldedLiterals {
        code: applied.output,
        constructions: count,
        arrays,
        writes,
    })
}

struct Objects<'s, 'a> {
    source: &'s str,
    protected: &'s BTreeSet<String>,
    scoping: &'s Scoping,
    nodes: &'s oxc_semantic::AstNodes<'a>,
    next_field: usize,
    edits: Vec<analysis::Edit>,
    dynamic: bool,
    arrays: BTreeSet<u32>,
}
impl Objects<'_, '_> {
    fn text(&self, s: Span) -> &str {
        &self.source[s.start as usize..s.end as usize]
    }
    fn write<'e, 'a>(
        &self,
        s: &'e Statement<'a>,
        symbol: SymbolId,
    ) -> Option<(Span, Span, Span, bool, Option<u32>)> {
        let Statement::ExpressionStatement(statement) = s else {
            return None;
        };
        let Expression::CallExpression(call) = unwrap_parens(&statement.expression) else {
            return None;
        };
        if call.optional
            || call.arguments.len() != 3
            || call.arguments.iter().any(|a| a.is_spread())
        {
            return None;
        }
        let callee = unwrap_parens(&call.callee);
        let object = match callee {
            Expression::StaticMemberExpression(m)
                if !m.optional && m.property.name == "defineProperty" =>
            {
                &m.object
            }
            Expression::ComputedMemberExpression(m)
                if !m.optional
                    && matches!(&m.expression,Expression::StringLiteral(k) if k.value=="defineProperty") =>
            {
                &m.object
            }
            _ => return None,
        };
        let Expression::Identifier(builtin) = unwrap_parens(object) else {
            return None;
        };
        if builtin.name != "Object" || analysis::identifier_symbol(builtin, self.scoping).is_some()
        {
            return None;
        }
        let Expression::Identifier(receiver) = unwrap_parens(call.arguments[0].to_expression())
        else {
            return None;
        };
        if analysis::identifier_symbol(receiver, self.scoping) != Some(symbol) {
            return None;
        }
        let key = unwrap_parens(call.arguments[1].to_expression());
        if !matches!(
            key,
            Expression::StringLiteral(_) | Expression::NumericLiteral(_)
        ) {
            return None;
        }
        let descriptor = unwrap_parens(call.arguments[2].to_expression());
        let info = exact_data_descriptor(descriptor).ok()?;
        Some((
            receiver.span,
            key.span(),
            info.value_span,
            matches!(key,Expression::StringLiteral(k) if k.value=="__proto__"),
            static_property_key(key).and_then(|key| key.array_index),
        ))
    }

    fn body(&mut self, body: &[Statement<'_>]) {
        for (index, s) in body.iter().enumerate() {
            let Statement::VariableDeclaration(d) = s else {
                continue;
            };
            if d.kind != VariableDeclarationKind::Let || d.declarations.len() != 1 {
                continue;
            }
            let item = &d.declarations[0];
            let BindingPattern::BindingIdentifier(id) = &item.id else {
                continue;
            };
            if !["$SSA_", "$OBJECT_", "$ARRAY_"]
                .iter()
                .any(|p| id.name.starts_with(p))
                || self.protected.contains(id.name.as_str())
            {
                continue;
            }
            let array = match &item.init {
                Some(Expression::ObjectExpression(object)) if object.properties.is_empty() => false,
                Some(Expression::ArrayExpression(array)) if array.elements.is_empty() => true,
                _ => continue,
            };
            let Some(symbol) = id.symbol_id.get() else {
                continue;
            };
            let refs = self
                .scoping
                .get_resolved_references(symbol)
                .collect::<Vec<_>>();
            if refs.iter().any(|r| r.is_write()) {
                continue;
            }
            let mut writes = Vec::new();
            for next in &body[index + 1..] {
                if let Some(write) = self.write(next, symbol) {
                    writes.push((next.span(), write));
                    continue;
                }
                // Only immutable SSA evaluations may separate construction writes.
                if let Statement::VariableDeclaration(next) = next {
                    if next.kind!=VariableDeclarationKind::Var&&next.declarations.iter().all(|v|matches!(&v.id,BindingPattern::BindingIdentifier(id) if id.name.starts_with("$SSA_") && id.symbol_id.get().is_some_and(|s|!self.scoping.get_resolved_references(s).any(|r|r.is_write())))) {continue;}
                }
                break;
            }
            if writes.is_empty()
                || (array
                    && writes
                        .iter()
                        .enumerate()
                        .any(|(index, (_, write))| write.4 != Some(index as u32)))
            {
                continue;
            }
            let end = writes.last().expect("at least two writes").0.end;
            let targets = writes.iter().map(|(_, w)| w.0).collect::<Vec<_>>();
            // Every use before completion must be exactly a descriptor receiver.
            // This rejects self-reference, escape, closure capture and observers.
            if refs.iter().any(|r| {
                let span = self.nodes.get_node(r.node_id()).kind().span();
                span.start < end && !targets.contains(&span)
            }) {
                continue;
            }
            let mut capture = Capture {
                symbol,
                scoping: self.scoping,
                nested: 0,
                captured: false,
            };
            for next in body {
                capture.visit_statement(next);
            }
            if capture.captured {
                continue;
            }
            let mut fields = Vec::new();
            let mut snapshots = Vec::new();
            for (_, (_, key, value, proto, _)) in &writes {
                let name = loop {
                    let candidate = format!("$SSA_OBJECT_FIELD_{}", self.next_field);
                    self.next_field += 1;
                    if !self.source.contains(&candidate) {
                        break candidate;
                    }
                };
                let key_text = self.text(*key);
                let key_text = if *proto {
                    format!("[{key_text}]")
                } else {
                    key_text.to_owned()
                };
                fields.push(if array {
                    name.clone()
                } else {
                    format!("{key_text}: {name}")
                });
                snapshots.push(format!("let {name} = ({});", self.text(*value)));
            }
            let replacement = if array {
                format!("let {} = [{}];", id.name, fields.join(", "))
            } else {
                format!("let {} = {{{}}};", id.name, fields.join(", "))
            };
            self.edits.push(analysis::Edit::delete(d.span));
            if array {
                self.arrays.insert(d.span.start);
            }
            for (at, (span, _)) in writes.iter().enumerate() {
                let mut text = snapshots[at].clone();
                if at + 1 == writes.len() {
                    text.push_str(&replacement);
                }
                self.edits.push(analysis::Edit::replace(*span, text));
            }
        }
    }
}
struct Capture<'s> {
    symbol: SymbolId,
    scoping: &'s Scoping,
    nested: usize,
    captured: bool,
}
impl<'a> Visit<'a> for Capture<'_> {
    fn visit_function(&mut self, f: &Function<'a>, flags: ScopeFlags) {
        self.nested += 1;
        walk::walk_function(self, f, flags);
        self.nested -= 1;
    }
    fn visit_arrow_function_expression(&mut self, f: &ArrowFunctionExpression<'a>) {
        self.nested += 1;
        walk::walk_arrow_function_expression(self, f);
        self.nested -= 1;
    }
    fn visit_class(&mut self, c: &Class<'a>) {
        self.nested += 1;
        walk::walk_class(self, c);
        self.nested -= 1;
    }
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        self.captured |=
            self.nested > 0 && analysis::identifier_symbol(id, self.scoping) == Some(self.symbol);
    }
}
impl<'a> Visit<'a> for Objects<'_, '_> {
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        self.dynamic |= call
            .arguments
            .get(1)
            .and_then(|a| a.as_expression())
            .is_some_and(|e| matches!(e,Expression::StringLiteral(k) if k.value=="defineProperty"));
        walk::walk_call_expression(self, call);
    }
    fn visit_assignment_target(&mut self, target: &AssignmentTarget<'a>) {
        self.dynamic |= match target {
            AssignmentTarget::StaticMemberExpression(m) => m.property.name == "defineProperty",
            AssignmentTarget::ComputedMemberExpression(m) => {
                matches!(&m.expression,Expression::StringLiteral(k) if k.value=="defineProperty")
            }
            AssignmentTarget::AssignmentTargetIdentifier(id) => {
                id.name == "Object" && analysis::identifier_symbol(id, self.scoping).is_none()
            }
            _ => false,
        };
        walk::walk_assignment_target(self, target);
    }
    fn visit_function_body(&mut self, b: &FunctionBody<'a>) {
        self.body(&b.statements);
        walk::walk_function_body(self, b);
    }
    fn visit_block_statement(&mut self, b: &BlockStatement<'a>) {
        self.body(&b.body);
        walk::walk_block_statement(self, b);
    }
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        self.dynamic |= id.name == "eval";
    }
    fn visit_with_statement(&mut self, s: &WithStatement<'a>) {
        self.dynamic = true;
        walk::walk_with_statement(self, s);
    }
}
