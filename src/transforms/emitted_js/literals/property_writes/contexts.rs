//! Where receivers are allocated and used: contexts, blocks and allocation literals.

use super::*;

#[derive(Debug, Clone, Copy)]
pub(super) struct DirectContext {
    pub(super) block_id: usize,
    pub(super) statement_index: usize,
}

#[derive(Debug, Default)]
pub(super) struct ContextCollector {
    pub(super) blocks: Vec<BlockInfo>,
    pub(super) declarations: BTreeMap<SymbolId, AllocationPlan>,
    pub(super) duplicate_symbols: BTreeSet<SymbolId>,
    pub(super) parameter_symbols: BTreeSet<SymbolId>,
    pub(super) call_contexts: BTreeMap<u32, DirectContext>,
    pub(super) program_block_id: Option<usize>,
}

#[derive(Debug)]
pub(super) struct BlockInfo {
    pub(super) statement_spans: Vec<Span>,
    #[allow(dead_code)]
    pub(super) allow_var: bool,
}

#[derive(Debug, Clone)]
pub(super) struct AllocationPlan {
    pub(super) block_id: usize,
    pub(super) statement_index: usize,
    pub(super) literal: AllocationLiteral,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ArrayElementSource {
    pub(super) span: Span,
    pub(super) needs_parentheses: bool,
}

#[derive(Debug, Clone)]
pub(super) enum AllocationLiteral {
    Array {
        span: Span,
        /// `Some(span)` for an explicit element, `None` for an elision/hole.
        elements: Vec<Option<ArrayElementSource>>,
        existing: BTreeSet<String>,
        preseedable: bool,
    },
    Object {
        span: Span,
        existing: BTreeSet<String>,
        preseedable: bool,
        property_count: usize,
        last_property_end: Option<u32>,
    },
}

impl ContextCollector {
    pub(super) fn collect_parameters(&mut self, parameters: &FormalParameters<'_>) {
        for pattern in parameters.iter_bindings() {
            for binding in pattern.get_binding_identifiers() {
                if let Some(symbol) = binding.symbol_id.get() {
                    self.parameter_symbols.insert(symbol);
                }
            }
        }
    }

    pub(super) fn scan_statements(&mut self, statements: &[Statement<'_>], allow_var: bool) -> usize {
        let block_id = self.blocks.len();
        let mut statement_spans = Vec::with_capacity(statements.len());
        for (index, statement) in statements.iter().enumerate() {
            statement_spans.push(statement.span());
            if let Statement::ExpressionStatement(expression) = statement {
                if let Expression::CallExpression(call) = unwrap_parens(&expression.expression) {
                    self.call_contexts.insert(
                        call.span.start,
                        DirectContext {
                            block_id,
                            statement_index: index,
                        },
                    );
                }
            }
            if let Statement::VariableDeclaration(declaration) = statement {
                self.collect_declaration(declaration, block_id, index, allow_var);
            }
        }
        self.blocks.push(BlockInfo {
            statement_spans,
            allow_var,
        });
        block_id
    }

    pub(super) fn collect_declaration(
        &mut self,
        declaration: &VariableDeclaration<'_>,
        block_id: usize,
        statement_index: usize,
        allow_var: bool,
    ) {
        if declaration.declare {
            return;
        }
        if !allow_var && declaration.kind == VariableDeclarationKind::Var {
            // Program-level `var` bindings are global object properties and are
            // observable outside this script.
            return;
        }
        for declarator in &declaration.declarations {
            let BindingPattern::BindingIdentifier(binding) = &declarator.id else {
                continue;
            };
            let Some(symbol) = binding.symbol_id.get() else {
                continue;
            };
            let Some(initializer) = &declarator.init else {
                continue;
            };
            let Some(literal) = allocation_literal(unwrap_parens(initializer)) else {
                continue;
            };
            match self.declarations.entry(symbol) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(AllocationPlan {
                        block_id,
                        statement_index,
                        literal,
                    });
                }
                std::collections::btree_map::Entry::Occupied(_) => {
                    self.duplicate_symbols.insert(symbol);
                }
            }
        }
    }
}

impl<'a> Visit<'a> for ContextCollector {
    fn visit_program(&mut self, program: &Program<'a>) {
        self.program_block_id = Some(self.scan_statements(&program.body, false));
        walk::walk_program(self, program);
    }

    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        self.collect_parameters(&function.params);
        if let Some(body) = &function.body {
            self.scan_statements(&body.statements, true);
        }
        walk::walk_function(self, function, flags);
    }

    fn visit_arrow_function_expression(&mut self, arrow: &ArrowFunctionExpression<'a>) {
        self.collect_parameters(&arrow.params);
        self.scan_statements(&arrow.body.statements, true);
        walk::walk_arrow_function_expression(self, arrow);
    }

    fn visit_block_statement(&mut self, block: &BlockStatement<'a>) {
        self.scan_statements(&block.body, true);
        walk::walk_block_statement(self, block);
    }

    fn visit_static_block(&mut self, block: &StaticBlock<'a>) {
        self.scan_statements(&block.body, true);
        walk::walk_static_block(self, block);
    }
}

pub(super) fn allocation_literal(expression: &Expression<'_>) -> Option<AllocationLiteral> {
    match expression {
        Expression::ArrayExpression(array) => {
            let mut elements = Vec::with_capacity(array.elements.len());
            let mut existing = BTreeSet::new();
            let mut preseedable = true;
            for (index, element) in array.elements.iter().enumerate() {
                match element {
                    ArrayExpressionElement::Elision(_) => elements.push(None),
                    ArrayExpressionElement::SpreadElement(_) => {
                        preseedable = false;
                        elements.push(None);
                    }
                    _ => {
                        elements.push(Some(ArrayElementSource {
                            span: element.span(),
                            needs_parentheses: matches!(
                                element,
                                ArrayExpressionElement::SequenceExpression(_)
                            ),
                        }));
                        // Indices after a spread are not their AST positions,
                        // so only the fixed prefix contributes known keys.
                        if preseedable {
                            existing.insert(index.to_string());
                        }
                    }
                }
            }
            Some(AllocationLiteral::Array {
                span: array.span,
                elements,
                existing,
                preseedable,
            })
        }
        Expression::ObjectExpression(object) => {
            let mut existing = BTreeSet::new();
            let mut preseedable = true;
            let mut last_property_end = None;
            for property in &object.properties {
                match property {
                    ObjectPropertyKind::SpreadProperty(_) => {
                        preseedable = false;
                        last_property_end = Some(property.span().end);
                    }
                    ObjectPropertyKind::ObjectProperty(property) => {
                        last_property_end = Some(property.span.end);
                        if property.kind != PropertyKind::Init {
                            preseedable = false;
                            if let Some(key) =
                                object_literal_property_key(&property.key, property.computed)
                            {
                                existing.remove(&key);
                            } else {
                                // A dynamic accessor may replace any earlier data
                                // property, so no prior own-data fact remains safe.
                                existing.clear();
                            }
                            continue;
                        }
                        let Some(key) =
                            object_literal_property_key(&property.key, property.computed)
                        else {
                            preseedable = false;
                            continue;
                        };
                        if key == "__proto__" {
                            // Non-computed `__proto__` can set the prototype; a
                            // computed one is an own property, but the whole
                            // literal is retained to keep the proof simple.
                            preseedable = false;
                            continue;
                        }
                        existing.insert(key);
                    }
                }
            }
            Some(AllocationLiteral::Object {
                span: object.span,
                existing,
                preseedable,
                property_count: object.properties.len(),
                last_property_end,
            })
        }
        _ => None,
    }
}

pub(super) fn object_literal_property_key(key: &PropertyKey<'_>, computed: bool) -> Option<String> {
    match key {
        PropertyKey::StaticIdentifier(identifier) if !computed => Some(identifier.name.to_string()),
        PropertyKey::StringLiteral(literal) if !literal.lone_surrogates => {
            Some(literal.value.to_string())
        }
        PropertyKey::NumericLiteral(literal) => Some(number_to_js_string(literal.value)),
        _ => None,
    }
}
