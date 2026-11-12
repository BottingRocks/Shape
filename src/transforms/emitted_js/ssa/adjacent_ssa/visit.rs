//! The walk that feeds the scan, and the visitors it consults.

use super::*;

pub(super) struct InsertedScope<'s> {
    pub(super) scoping: &'s Scoping,
    pub(super) scope: oxc_semantic::ScopeId,
    pub(super) initializer: Span,
    pub(super) valid: bool,
}
impl<'a> Visit<'a> for InsertedScope<'_> {
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        let symbol = analysis::identifier_symbol(id, self.scoping);
        if symbol.is_some_and(|s| {
            let at = self.scoping.symbol_span(s);
            self.initializer.start <= at.start && at.end <= self.initializer.end
        }) {
            return;
        }
        self.valid &= self.scoping.find_binding(self.scope, id.name) == symbol;
    }
}

#[derive(Default)]
pub(super) struct DirectCallCallees {
    pub(super) spans: Vec<Span>,
    pub(super) typeof_operands: Vec<Span>,
}

impl<'a> Visit<'a> for DirectCallCallees {
    fn visit_unary_expression(&mut self, unary: &UnaryExpression<'a>) {
        if unary.operator == UnaryOperator::Typeof {
            let mut value = &unary.argument;
            while let Expression::ParenthesizedExpression(p) = value {
                value = &p.expression;
            }
            if let Expression::Identifier(id) = value {
                self.typeof_operands.push(id.span);
            }
        }
        walk::walk_unary_expression(self, unary);
    }
    fn visit_tagged_template_expression(&mut self, tag: &TaggedTemplateExpression<'a>) {
        let mut callee = &tag.tag;
        while let Expression::ParenthesizedExpression(p) = callee {
            callee = &p.expression;
        }
        if let Expression::Identifier(id) = callee {
            self.spans.push(id.span);
        }
        walk::walk_tagged_template_expression(self, tag);
    }
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        let mut callee = &call.callee;
        while let Expression::ParenthesizedExpression(p) = callee {
            callee = &p.expression;
        }
        if let Expression::Identifier(id) = callee {
            self.spans.push(id.span);
        }
        walk::walk_call_expression(self, call);
    }
}

impl<'a> Visit<'a> for Scan<'_> {
    fn visit_sequence_expression(&mut self, sequence: &SequenceExpression<'a>) {
        if self.native_calls {
            for (position, first) in sequence.expressions.iter().enumerate() {
                let Expression::AssignmentExpression(store) = first else {
                    continue;
                };
                let AssignmentTarget::AssignmentTargetIdentifier(id) = &store.left else {
                    continue;
                };
                let Some(symbol) = analysis::identifier_symbol(id, self.scoping) else {
                    continue;
                };
                if store.operator != AssignmentOperator::Assign
                    || self.protected.contains(id.name.as_str())
                    || self.scoping.symbol_flags(symbol).is_const_variable()
                    || !self.scoping.symbol_redeclarations(symbol).is_empty()
                    || !self.local_reads.initialized(symbol, store.span)
                {
                    continue;
                }
                // Keep loop-carried or captured cells. The only references
                // must be this store and one read in the next comma operand.
                let refs: Vec<_> = self.scoping.get_resolved_references(symbol).collect();
                if refs.len() != 2 || refs.iter().filter(|r| r.is_write()).count() != 1 {
                    continue;
                }
                let AstKind::VariableDeclarator(declaration) =
                    self.nodes.kind(self.scoping.symbol_declaration(symbol))
                else {
                    continue;
                };
                if declaration.init.is_some()
                    || !matches!(&declaration.id, BindingPattern::BindingIdentifier(binding)
                        if self.scoping.symbol_span(symbol) == binding.span)
                {
                    continue;
                }
                let mut initializer = &store.right;
                while let Expression::ParenthesizedExpression(p) = initializer {
                    initializer = &p.expression;
                }
                if matches!(
                    initializer,
                    Expression::FunctionExpression(_)
                        | Expression::ArrowFunctionExpression(_)
                        | Expression::ClassExpression(_)
                ) {
                    continue;
                }
                let mut readable = BTreeSet::new();
                self.local_reads.readable(store.right.span(), &mut readable);
                let commuting =
                    self.local_reads
                        .commuting_read(&store.right, self.scoping, self.protected);
                if commuting.is_some() {
                    self.local_reads
                        .plain_reads(store.right.span(), &mut readable);
                }
                let policy = ReadPolicy {
                    commuting_initialized_read: commuting == Some(true),
                    this_initialized: self.local_reads.classes.enclosing(store.span).is_none(),
                    primitive_inputs: &self.local_reads.primitive_inputs,
                    cells: self.cells,
                    ordinary_number: self.local_reads.ordinary_number,
                };
                let mut consumer = None;
                for next in sequence.expressions.iter().skip(position + 1) {
                    if let Ok(Some(read)) = prefix_read(
                        next,
                        symbol,
                        self.scoping,
                        &readable,
                        true,
                        self.protected,
                        policy,
                    ) {
                        consumer = Some((read, next));
                        break;
                    }
                    if !self.independent_comma(&store.right, next, symbol, &readable, policy) {
                        break;
                    }
                }
                let Some((read, next)) = consumer else {
                    continue;
                };
                if !refs.iter().any(|r| {
                    r.is_read()
                        && !r.is_write()
                        && self.nodes.get_node(r.node_id()).kind().span() == read.span
                }) {
                    continue;
                }
                let mut contexts = DirectCallCallees::default();
                contexts.visit_expression(next);
                // The snapshot already performed GetValue. Keep detached
                // receivers and indirect eval, and do not let typeof suppress
                // a formerly throwing unresolved read.
                let force_value = contexts.spans.contains(&read.span)
                    || contexts.typeof_operands.contains(&read.span);
                let removal = first.span();
                let removed = &self.source[removal.start as usize..removal.end as usize];
                if removed.contains("/*") || removed.contains("//") {
                    continue;
                }
                let proposal = [
                    // Keep comma GetValue semantics when an outer call uses
                    // a member-valued last operand as its detached callee.
                    // This also retains all operand parentheses.
                    analysis::Edit::replace(removal, "0"),
                    analysis::Edit::replace(read.span, {
                        let rhs = &self.source
                            [store.right.span().start as usize..store.right.span().end as usize];
                        if force_value {
                            format!("(0, ({rhs}))")
                        } else {
                            format!("({rhs})")
                        }
                    }),
                ];
                if proposal.iter().any(|e| {
                    self.edits
                        .iter()
                        .any(|p| e.start < p.end && p.start < e.end)
                }) {
                    continue;
                }
                self.edits.extend(proposal);
                self.inlined += 1;
            }
        }
        walk::walk_sequence_expression(self, sequence);
    }

    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        if let Some(body) = &function.body {
            self.body(&body.statements, true);
        }
        walk::walk_function(self, function, flags);
    }

    fn visit_block_statement(&mut self, block: &BlockStatement<'a>) {
        self.body(&block.body, false);
        walk::walk_block_statement(self, block);
    }

    fn visit_arrow_function_expression(&mut self, f: &ArrowFunctionExpression<'a>) {
        if !f.expression {
            self.body(&f.body.statements, true);
        }
        walk::walk_arrow_function_expression(self, f);
    }

    fn visit_with_statement(&mut self, statement: &WithStatement<'a>) {
        self.dynamic_scope = true;
        walk::walk_with_statement(self, statement);
    }

    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        if identifier.name == "eval" {
            self.dynamic_scope = true;
        }
    }
}
