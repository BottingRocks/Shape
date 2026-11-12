use super::{algorithms::AlgorithmReport, consumer::WireError};
use crate::signals::{
    codec::{SignalReader, SignalWriter},
    extract::contracts::{EmissionContract, GrammarNode, LoopDirection, MaskForm, WriteNode},
};
use oxc_allocator::Allocator;
use oxc_ast::ast::Expression;
use oxc_parser::Parser;
use oxc_span::SourceType;
use serde::{Deserialize, Serialize};

/// Numbers use IEEE bits, retaining negative zero and nonfinite encodings.
/// Arrays are in source iteration order; the program records direction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum WireValue {
    Byte(u8),
    NumberBits(u64),
    StringUnits(Vec<u16>),
    Sequence(Vec<WireValue>),
    Repeat(Vec<WireValue>),
    Choice { arm: usize, value: Box<WireValue> },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum WireMask {
    Inherit,
    None,
    Nested { salt: u32 },
    Unresolved,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Op {
    RootChoice {
        cases: Vec<RootCase>,
    },
    Byte {
        mask: WireMask,
        xor: u8,
        allowed: Option<Vec<u8>>,
    },
    Number {
        #[serde(default)]
        envelope: Option<super::control::NumberEnvelope>,
        mask: WireMask,
        literal: Option<u64>,
    },
    String {
        mask: WireMask,
        literal: Option<Vec<u16>>,
    },
    Sequence {
        items: Vec<Op>,
    },
    Repeat {
        count: usize,
        direction: LoopDirection,
        body: Box<Op>,
    },
    CountedRepeat {
        count: super::CountExpr,
        direction: LoopDirection,
        body: Box<Op>,
    },
    CounterChoice {
        proof: super::control::CounterChoiceProof,
        arms: Vec<Op>,
    },
    DomainChoice {
        source: crate::signals::schema::SourceSpan,
        path: super::control::ControlPath,
        consumption: TagConsumption,
        semantics: Option<super::control::ChoiceSemantics>,
        domains: Vec<Vec<u8>>,
        mask: WireMask,
        arms: Vec<Op>,
    },
    /// Every arm starts with a distinct literal byte. The tag is consumed
    /// exactly once by that arm; no speculative reads or trial decoding.
    Choice {
        discriminants: Vec<Vec<u8>>,
        mask: WireMask,
        xor: u8,
        arms: Vec<Op>,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TagConsumption {
    ArmOwnsTag,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Program {
    pub prefix_byte_order: String,
    pub seed_source: String,
    pub stream_scope: String,
    pub string_scope: String,
    pub salt: u32,
    pub body: Op,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum Support {
    /// Structurally verified IR; execution requires a source-bound witness for
    /// this emission in the supplied public initialization epoch.
    RequiresInitializer {
        program: Program,
        stream_body: super::AlgorithmProof,
        required_context: super::InitializationEpoch,
    },

    Supported {
        program: Program,
    },
    Unsupported {
        reason: String,
        minimum_input: Option<String>,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RootCase {
    pub plan: super::control::RootCasePlan,
    pub support: Box<Support>,
}
fn unsupported(reason: &str) -> WireError {
    WireError::Unsupported(reason.into())
}
fn source_float_prefix(source: &str, algorithms: &AlgorithmReport) -> bool {
    let proof = &algorithms.float_prefix;
    proof.codec.is_some()
        && proof
            .span
            .and_then(|span| source.get(span.start as usize..span.end as usize))
            .is_some_and(|body| {
                proof.sha256.as_deref() == Some(super::algorithms::digest(body.as_bytes()).as_str())
            })
}

pub(super) fn compile(
    emission: &EmissionContract,
    algorithms: &AlgorithmReport,
    source: &str,
) -> Result<Program, WireError> {
    compile_with_stream(emission, algorithms, source, &algorithms.stream)
}

pub(super) fn compile_with_stream(
    emission: &EmissionContract,
    algorithms: &AlgorithmReport,
    source: &str,
    stream_body: &super::AlgorithmProof,
) -> Result<Program, WireError> {
    compile_in_scope(emission, algorithms, source, stream_body, None)
}
fn compile_in_scope(
    emission: &EmissionContract,
    algorithms: &AlgorithmReport,
    source: &str,
    stream_body: &super::AlgorithmProof,
    root_case: Option<&super::RootCasePlan>,
) -> Result<Program, WireError> {
    let root_scope = match root_case {
        Some(case) => Some(
            super::control::root_scope(source, case, emission.site)
                .ok_or_else(|| unsupported("root case source branch context is not proven"))?,
        ),
        None => None,
    };
    if let Some(candidates) = super::control::root_emissions(source, emission) {
        let cases = candidates
            .into_iter()
            .map(|(plan, e)| {
                let support =
                    match compile_in_scope(&e, algorithms, source, stream_body, Some(&plan)) {
                        Ok(program) => Support::Supported { program },
                        Err(error) => Support::Unsupported {
                            reason: error.to_string(),
                            minimum_input: Some(
                                "certified branch-local stream/string scope".into(),
                            ),
                        },
                    };
                RootCase {
                    plan,
                    support: Box::new(support),
                }
            })
            .collect();
        return Ok(Program {
            prefix_byte_order: "be32".into(),
            seed_source: "captured_prefix".into(),
            stream_scope: "source_root_case".into(),
            string_scope: "source_root_case".into(),
            salt: 0,
            body: Op::RootChoice { cases },
        });
    }
    let items = match &emission.grammar {
        GrammarNode::Sequence { items } => items.as_slice(),
        other => std::slice::from_ref(other),
    };
    let Some(GrammarNode::RawPrefix(prefix)) = items.first() else {
        return Err(unsupported(
            "emission does not start with one proven raw prefix",
        ));
    };
    if algorithms.hash.codec.is_none() || stream_body.codec.is_none() {
        return Err(unsupported(
            "source token hash or stream algorithm is not certified",
        ));
    }
    let stream = emission
        .stream
        .as_ref()
        .ok_or_else(|| unsupported("stream seed/salt is unresolved"))?;
    if prefix.seed_binding.is_none()
        || prefix.seed_binding != stream.seed_binding
        || !prefix_be32(&prefix.elements)
    {
        return Err(unsupported(
            "prefix byte order or stream/prefix seed binding is unresolved",
        ));
    }
    let salt = stream
        .salt
        .as_deref()
        .and_then(crate::signals::schema::parse_salt)
        .and_then(|s| {
            u32::try_from(s)
                .ok()
                .or_else(|| i32::try_from(s).ok().map(|s| s as u32))
        })
        .ok_or_else(|| unsupported("stream salt is unresolved"))?;
    let source_plan = super::control::source_plan(source, emission, algorithms);
    let body = Op::Sequence {
        items: items[1..]
            .iter()
            .map(|n| compile_node(n, emission, algorithms, &source_plan, source, 0))
            .collect::<Result<_, _>>()?,
    };
    if !local_scopes(source, emission, algorithms, root_scope.as_ref()) {
        return Err(unsupported(
            "source resource lifetime/read accounting is not proven for this emission",
        ));
    }
    if !super::position::validate_in_scope(
        source,
        emission,
        root_scope.as_ref(),
        Some(&source_plan),
    ) {
        return Err(unsupported(
            "physical write positions are not proven contiguous from the prefix",
        ));
    }
    Ok(Program {
        prefix_byte_order: "be32".into(),
        seed_source: "captured_prefix".into(),
        stream_scope: "emission_shared_cursor".into(),
        string_scope: "emission_local_writer_instance".into(),
        salt,
        body,
    })
}
fn local_scopes(
    source: &str,
    emission: &EmissionContract,
    algorithms: &AlgorithmReport,
    root_scope: Option<&super::control::RootScope>,
) -> bool {
    use oxc_ast::ast::*;
    use oxc_ast_visit::{walk, Visit};
    use oxc_span::{GetSpan, Span};
    let mut instances = Vec::new();
    let mut missing = false;
    let mut writes = Vec::new();
    if let Some(instance) = emission.stream.as_ref().and_then(|s| s.instance) {
        instances.push(instance);
    } else {
        return false;
    }
    let mut string_instances = Vec::new();
    emission.grammar.walk(&mut |n| {
        if let GrammarNode::String(w) = n {
            if let Some(i) = w.string_instance {
                if !string_instances.contains(&i) {
                    string_instances.push(i);
                }
            } else {
                missing = true;
            }
        }
        if matches!(
            n,
            GrammarNode::Byte(_) | GrammarNode::Number(_) | GrammarNode::String(_)
        ) {
            writes.push(n);
        }
    });
    if missing || string_instances.len() > 1 {
        return false;
    }
    instances.extend(string_instances);
    struct CallSite {
        span: Span,
        function: Span,
        symbol: Option<oxc_semantic::SymbolId>,
        arguments: Vec<Span>,
        receiverless: bool,
    }
    struct Boundaries<'s> {
        scopes: &'s oxc_semantic::Scoping,
        functions: Vec<Span>,
        blocks: Vec<Span>,
        loops: Vec<(Span, Span)>,
        bindings: Vec<(Span, oxc_semantic::SymbolId, Option<Span>)>,
        dynamic: Vec<Span>,
        calls: Vec<CallSite>,
    }
    impl<'a> Visit<'a> for Boundaries<'_> {
        fn visit_function(&mut self, f: &Function<'a>, flags: oxc_syntax::scope::ScopeFlags) {
            self.functions.push(f.span);
            if let Some(b) = &f.body {
                self.blocks.push(b.span);
            }
            walk::walk_function(self, f, flags);
        }
        fn visit_arrow_function_expression(&mut self, f: &ArrowFunctionExpression<'a>) {
            self.functions.push(f.span);
            self.blocks.push(f.body.span);
            walk::walk_arrow_function_expression(self, f);
        }
        fn visit_block_statement(&mut self, b: &BlockStatement<'a>) {
            self.blocks.push(b.span);
            walk::walk_block_statement(self, b);
        }
        fn visit_for_statement(&mut self, n: &ForStatement<'a>) {
            self.loops.push((n.span, n.body.span()));
            walk::walk_for_statement(self, n);
        }
        fn visit_for_in_statement(&mut self, n: &ForInStatement<'a>) {
            self.loops.push((n.span, n.body.span()));
            walk::walk_for_in_statement(self, n);
        }
        fn visit_for_of_statement(&mut self, n: &ForOfStatement<'a>) {
            self.loops.push((n.span, n.body.span()));
            walk::walk_for_of_statement(self, n);
        }
        fn visit_while_statement(&mut self, n: &WhileStatement<'a>) {
            self.loops.push((n.span, n.body.span()));
            walk::walk_while_statement(self, n);
        }
        fn visit_do_while_statement(&mut self, n: &DoWhileStatement<'a>) {
            self.loops.push((n.span, n.body.span()));
            walk::walk_do_while_statement(self, n);
        }
        fn visit_call_expression(&mut self, c: &CallExpression<'a>) {
            let (function, receiverless) = match c.callee.without_parentheses() {
                Expression::StaticMemberExpression(m) if m.property.name == "call" => {
                    (&m.object, true)
                }
                other => (other, false),
            };
            let symbol = if let Expression::Identifier(id) = function.without_parentheses() {
                id.reference_id
                    .get()
                    .and_then(|r| self.scopes.get_reference(r).symbol_id())
            } else {
                None
            };
            self.calls.push(CallSite {
                span: c.span,
                function: function.span(),
                symbol,
                arguments: c.arguments.iter().map(GetSpan::span).collect(),
                receiverless,
            });
            if matches!(c.callee.without_parentheses(),Expression::Identifier(i) if i.name=="eval")
            {
                self.dynamic.push(c.span);
            }
            walk::walk_call_expression(self, c);
        }
        fn visit_variable_declarator(&mut self, d: &VariableDeclarator<'a>) {
            if let (BindingPattern::BindingIdentifier(id), Some(init)) = (&d.id, &d.init) {
                if let Some(s) = id.symbol_id.get() {
                    self.bindings.push((init.span(), s, None));
                }
            }
            walk::walk_variable_declarator(self, d);
        }
        fn visit_expression_statement(&mut self, statement: &ExpressionStatement<'a>) {
            if let Expression::AssignmentExpression(e) = statement.expression.without_parentheses()
            {
                if e.operator == oxc_syntax::operator::AssignmentOperator::Assign {
                    if let AssignmentTarget::AssignmentTargetIdentifier(id) = &e.left {
                        if let Some(s) = id
                            .reference_id
                            .get()
                            .and_then(|r| self.scopes.get_reference(r).symbol_id())
                        {
                            self.bindings.push((e.right.span(), s, Some(id.span)));
                        }
                    }
                }
            }
            walk::walk_expression_statement(self, statement);
        }
        fn visit_with_statement(&mut self, w: &WithStatement<'a>) {
            self.dynamic.push(w.span);
            walk::walk_with_statement(self, w);
        }
    }
    let arena = Allocator::default();
    let parsed = Parser::new(&arena, source, SourceType::mjs()).parse();
    if !parsed.errors.is_empty() {
        return false;
    }
    let semantic = oxc_semantic::SemanticBuilder::new()
        .build(&parsed.program)
        .semantic;
    let mut b = Boundaries {
        scopes: semantic.scoping(),
        functions: vec![],
        blocks: vec![],
        loops: vec![],
        bindings: vec![],
        dynamic: vec![],
        calls: vec![],
    };
    b.visit_program(&parsed.program);
    let site = Span::new(emission.site.start, emission.site.end);
    let contains = |outer: Span, inner: Span| outer.start <= inner.start && inner.end <= outer.end;
    let smallest = |spans: &[Span], inner: Span| {
        spans
            .iter()
            .copied()
            .filter(|s| contains(*s, inner))
            .min_by_key(|s| s.size())
    };
    let owner = smallest(&b.functions, site);
    let on_path = |span| root_scope.is_none_or(|scope| scope.includes(span));
    // A retained draw is one scalar value, not a fresh keystream byte at each
    // later read. The current byte IR cannot represent its reuse as a mask.
    // Check reaching values so a later plain reassignment is a separate value.
    for node in &writes {
        let GrammarNode::Byte(w) = node else { continue };
        let Some(draw) = w.stream_draw else { continue };
        let draw = Span::new(draw.start, draw.end);
        let Some((_, symbol, _)) = b.bindings.iter().find(|(value, _, _)| *value == draw) else {
            continue;
        };
        let mut uses = 0;
        for reference in b
            .scopes
            .get_resolved_references(*symbol)
            .filter(|r| r.is_read())
        {
            let at = semantic.nodes().get_node(reference.node_id()).span();
            if !on_path(at) {
                continue;
            }
            let oxc_ast::AstKind::IdentifierReference(identifier) =
                semantic.nodes().kind(reference.node_id())
            else {
                return false;
            };
            match crate::signals::extract::reaching::value_at_use(
                b.scopes,
                semantic.nodes(),
                identifier,
            ) {
                Ok(value) if value.value.span() != draw => continue,
                Ok(_) if contains(w.span, at) => uses += 1,
                _ => return false,
            }
        }
        if uses != 1 {
            return false;
        }
    }
    let mut prefix = None;
    emission.grammar.walk(&mut |node| {
        if let GrammarNode::RawPrefix(p) = node {
            if prefix.is_none() {
                prefix = Some(p);
            }
        }
    });
    let Some(prefix) = prefix else {
        return false;
    };
    let Some(seed) = prefix.seed_binding else {
        return false;
    };
    let Some(seed_symbol) = semantic.scoping().symbol_ids().find(|s| {
        let span = semantic.scoping().symbol_span(*s);
        span.start == seed.start && span.end == seed.end
    }) else {
        return false;
    };
    let stream = Span::new(instances[0].start, instances[0].end);
    let mut expected_reads = Vec::new();
    let mut resource_reads = Vec::new();
    // Only the actual mask/table input positions are permitted. Merely being
    // inside a writer's whole call span is insufficient: a value argument can
    // contain an extra discarded draw or leak the stream to an unknown call.
    for node in writes {
        let (GrammarNode::Byte(w) | GrammarNode::Number(w) | GrammarNode::String(w)) = node else {
            unreachable!()
        };
        if w.via_helper.is_some() {
            expected_reads.extend(w.helper_arguments.iter().map(|s| Span::new(s.start, s.end)));
            let instance = if matches!(node, GrammarNode::String(_)) {
                w.string_instance
            } else {
                w.stream_instance
            };
            if let Some(instance) = instance {
                resource_reads.extend(w.helper_arguments.iter().map(|argument| {
                    (
                        Span::new(argument.start, argument.end),
                        Span::new(instance.start, instance.end),
                    )
                }));
            }
            continue;
        }
        match node {
            GrammarNode::Byte(_) if w.mask == MaskForm::Stream => {
                let Some(instance) = w.stream_instance else {
                    return false;
                };
                let Some((_, symbol, _)) = b
                    .bindings
                    .iter()
                    .find(|(s, _, _)| s.start == instance.start && s.end == instance.end)
                else {
                    return false;
                };
                let calls: Vec<_> = b
                    .calls
                    .iter()
                    .filter(|c| {
                        (contains(w.span, c.span)
                            || w.stream_draw.is_some_and(|draw| {
                                draw.start == c.span.start && draw.end == c.span.end
                            }))
                            && c.symbol == Some(*symbol)
                            && c.arguments.is_empty()
                    })
                    .collect();
                if calls.len() != 1 {
                    return false;
                }
                expected_reads.push(calls[0].function);
                resource_reads.push((calls[0].function, Span::new(instance.start, instance.end)));
            }
            GrammarNode::Number(_) => {
                let Some(call) = b.calls.iter().find(|c| c.span == w.span) else {
                    return false;
                };
                if let Some(argument) = call.arguments.get(usize::from(call.receiverless)) {
                    expected_reads.push(*argument);
                    if let Some(instance) = w.stream_instance {
                        resource_reads.push((*argument, Span::new(instance.start, instance.end)));
                    }
                }
            }
            GrammarNode::String(_) => {
                let Some(call) = b.calls.iter().find(|c| c.span == w.span) else {
                    return false;
                };
                expected_reads.push(call.function);
                if let Some(instance) = w.string_instance {
                    resource_reads.push((call.function, Span::new(instance.start, instance.end)));
                }
            }
            _ => {}
        }
    }
    for instance in instances.iter().skip(1) {
        let Some(call) = b
            .calls
            .iter()
            .find(|c| c.span.start == instance.start && c.span.end == instance.end)
        else {
            return false;
        };
        if let Some(argument) = call.arguments.first() {
            expected_reads.push(*argument);
            resource_reads.push((*argument, stream));
        }
    }
    // A certified table factory only captures its stream; its returned closure
    // performs draws later. Account for a discarded allocation as a non-draw,
    // only through immutable source identities and an unused result.
    fn factory_origin(
        symbol: oxc_semantic::SymbolId,
        semantic: &oxc_semantic::Semantic<'_>,
        depth: usize,
    ) -> Option<Span> {
        use oxc_ast::AstKind;
        if depth > 16
            || !semantic.scoping().symbol_redeclarations(symbol).is_empty()
            || semantic
                .scoping()
                .get_resolved_references(symbol)
                .any(|r| r.is_write())
        {
            return None;
        }
        match semantic
            .nodes()
            .kind(semantic.scoping().symbol_declaration(symbol))
        {
            AstKind::Function(function) => Some(function.span),
            AstKind::VariableDeclarator(declarator) => {
                match declarator.init.as_ref()?.without_parentheses() {
                    Expression::FunctionExpression(function) => Some(function.span),
                    Expression::Identifier(id) => factory_origin(
                        id.reference_id
                            .get()
                            .and_then(|r| semantic.scoping().get_reference(r).symbol_id())?,
                        semantic,
                        depth + 1,
                    ),
                    _ => None,
                }
            }
            _ => None,
        }
    }
    if algorithms.string.codec.is_some() {
        for call in &b.calls {
            if call.receiverless
                || call.arguments.len() != 1
                || smallest(&b.functions, call.span) != owner
                || !algorithms.string.span.is_some_and(|span| {
                    call.symbol
                        .and_then(|symbol| factory_origin(symbol, &semantic, 0))
                        .is_some_and(|origin| {
                            origin.start == span.start
                                && origin.end <= span.end
                                && source
                                    .get(origin.end as usize..span.end as usize)
                                    .is_some_and(|tail| tail.trim().is_empty())
                        })
                })
            {
                continue;
            }
            let Some(node) = semantic.nodes().iter().find(|node| {
                matches!(node.kind(), oxc_ast::AstKind::CallExpression(_))
                    && node.span() == call.span
            }) else {
                continue;
            };
            if matches!(
                semantic.nodes().parent_kind(node.id()),
                oxc_ast::AstKind::ExpressionStatement(_)
            ) {
                expected_reads.push(call.arguments[0]);
                resource_reads.push((call.arguments[0], stream));
            }
        }
    }
    let seed_target = b
        .bindings
        .iter()
        .find(|(span, symbol, _)| *span == stream && *symbol == seed_symbol)
        .and_then(|(_, _, target)| *target);
    let seed_interval = Span::new(
        stream.start.min(prefix.span.start),
        stream.end.max(prefix.span.end),
    );
    if smallest(&b.functions, prefix.span) != owner
        || semantic
            .scoping()
            .get_resolved_references(seed_symbol)
            .any(|reference| {
                if !reference.is_write() {
                    return false;
                }
                let span = semantic.nodes().get_node(reference.node_id()).span();
                if !on_path(span) {
                    return false;
                }
                // `seed = factory(seed ^ salt)` assigns only after evaluating
                // its RHS. If the prefix was already written, reusing that
                // holder cannot change either captured seed input.
                (contains(seed_interval, span)
                    && !(Some(span) == seed_target && prefix.span.end <= stream.start))
                    || smallest(&b.functions, span) != owner
            })
    {
        return false;
    }
    if owner.is_none()
        || b.loops.iter().any(|(s, body)| {
            smallest(&b.functions, *s) == owner
                && contains(*s, site)
                && (!contains(*body, prefix.span)
                    || instances
                        .iter()
                        .any(|i| !contains(*body, Span::new(i.start, i.end))))
        })
        || b.dynamic
            .iter()
            .any(|s| owner.is_some_and(|o| contains(o, *s)))
    {
        return false;
    }
    // Check conditional ancestry for prefix and each resource independently.
    let dominates_expression = |expression: Span| {
        let Some(mut node) = semantic
            .nodes()
            .iter()
            .find(|n| n.span() == expression)
            .map(|n| n.id())
        else {
            return false;
        };
        loop {
            let parent = semantic.nodes().parent_id(node);
            if parent == node {
                break;
            }
            match semantic.nodes().kind(parent) {
                oxc_ast::AstKind::Function(_) | oxc_ast::AstKind::ArrowFunctionExpression(_) => {
                    break
                }
                oxc_ast::AstKind::IfStatement(statement) => {
                    let arm = if contains(statement.consequent.span(), expression) {
                        Some(statement.consequent.span())
                    } else {
                        statement
                            .alternate
                            .as_ref()
                            .map(GetSpan::span)
                            .filter(|span| contains(*span, expression))
                    };
                    if arm.is_none_or(|span| {
                        !contains(span, site)
                            && !root_scope.is_some_and(|scope| {
                                scope.choice == statement.span
                                    && scope.region == span
                                    && contains(span, expression)
                            })
                    }) {
                        return false;
                    }
                }
                oxc_ast::AstKind::TryStatement(statement) => {
                    let same_region = std::iter::once(statement.block.span)
                        .chain(statement.handler.as_ref().map(|handler| handler.body.span))
                        .chain(statement.finalizer.as_ref().map(|finally| finally.span))
                        .any(|region| contains(region, expression) && contains(region, site));
                    if !same_region {
                        return false;
                    }
                }
                oxc_ast::AstKind::ConditionalExpression(_)
                | oxc_ast::AstKind::LogicalExpression(_) => return false,
                _ => {}
            }
            node = parent;
        }
        true
    };
    if !dominates_expression(prefix.span) {
        return false;
    }
    instances.iter().all(|instance| {
        let instance = Span::new(instance.start, instance.end);
        if !dominates_expression(instance) {
            return false;
        }
        if owner != smallest(&b.functions, instance)
            || !smallest(&b.blocks, instance).is_some_and(|block| {
                contains(block, site)
                    || root_scope
                        .is_some_and(|scope| block == scope.region && contains(block, instance))
            })
            || instance.end >= site.start
        {
            return false;
        }
        // An immediately invoked certified string closure has an expression
        // lifetime: no holder, alias, or earlier table history exists.
        if instance != stream
            && b.calls
                .iter()
                .any(|call| call.function == instance && expected_reads.contains(&call.function))
        {
            return true;
        }
        let Some((_, symbol, target)) = b.bindings.iter().find(|(span, _, _)| *span == instance)
        else {
            return false;
        };
        // A plain assignment that is a direct statement in the constructor's
        // own block kills only this holder version, after evaluating its RHS.
        // Conditional/loop joins and aliases retain their proof obligations.
        let block = smallest(&b.blocks, instance);
        let kill = b
            .bindings
            .iter()
            .filter_map(|(rhs, holder, target)| {
                let target = (*target)?;
                if holder != symbol || rhs.start < instance.end || rhs.end >= site.start {
                    return None;
                }
                let node = semantic.nodes().iter().find(|node| node.span() == target)?;
                let assignment = semantic.nodes().parent_id(node.id());
                let statement = semantic.nodes().parent_id(assignment);
                if !matches!(
                    semantic.nodes().kind(statement),
                    oxc_ast::AstKind::ExpressionStatement(_)
                ) {
                    return None;
                }
                let parent = semantic.nodes().parent_id(statement);
                if Some(semantic.nodes().get_node(parent).span()) != block {
                    return None;
                }
                Some((target, rhs.end))
            })
            .min_by_key(|(_, end)| *end);
        // A later read of a recycled holder cannot inherit the old resource
        // identity merely because extraction retained a helper association.
        if kill.is_some_and(|(_, end)| {
            semantic
                .scoping()
                .get_resolved_references(*symbol)
                .filter(|reference| on_path(semantic.nodes().get_node(reference.node_id()).span()))
                .any(|reference| {
                    let span = semantic.nodes().get_node(reference.node_id()).span();
                    span.start >= end
                        && resource_reads
                            .iter()
                            .any(|(read, resource)| *read == span && *resource == instance)
                })
        }) {
            return false;
        }
        if semantic.scoping().symbol_flags(*symbol).is_const_variable()
            && semantic
                .scoping()
                .get_resolved_references(*symbol)
                .filter(|reference| on_path(semantic.nodes().get_node(reference.node_id()).span()))
                .any(|r| r.is_write())
        {
            return false;
        }
        // Every use of the captured state must be accounted for by this

        // emission's writes or its writer creation. Unmodelled draws, aliases,
        // escaped closures and other emissions retain a source-proof obligation.
        semantic
            .scoping()
            .get_resolved_references(*symbol)
            .filter(|reference| on_path(semantic.nodes().get_node(reference.node_id()).span()))
            .all(|reference| {
                let span = semantic.nodes().get_node(reference.node_id()).span();
                (kill.is_some_and(|(target, end)| Some(span) == Some(target) || span.start >= end)
                    && smallest(&b.functions, span) == owner)
                    || (smallest(&b.functions, span) == owner
                        && (span.end <= instance.start || span.start >= site.end))
                    || Some(span) == *target
                    || (reference.is_read()
                        && !reference.is_write()
                        && (contains(stream, span) || expected_reads.contains(&span)))
            })
    })
}
fn mask(w: &WriteNode, e: &EmissionContract) -> Result<WireMask, WireError> {
    match w.mask {
        MaskForm::Unmasked | MaskForm::StaticXor => Ok(WireMask::None),
        MaskForm::Stream
            if w.stream_instance.is_some()
                && w.stream_instance == e.stream.as_ref().and_then(|s| s.instance)
                && w.mask_source.as_ref() == e.stream.as_ref().map(|s| &s.argument) =>
        {
            Ok(WireMask::Inherit)
        }
        MaskForm::Stream => Err(unsupported(
            "nested stream identity/reset location is unresolved",
        )),
        MaskForm::Unresolved => Err(unsupported("leaf mask is unresolved")),
    }
}
fn allowed(w: &WriteNode) -> Option<Vec<u8>> {
    let arena = Allocator::default();
    let expression = Parser::new(&arena, &w.value, SourceType::mjs())
        .parse_expression()
        .ok()?;
    fn byte(e: &Expression<'_>) -> Option<u8> {
        if let Expression::NumericLiteral(n) = e.without_parentheses() {
            if n.value >= 0. && n.value <= 255. && n.value.fract() == 0. {
                return Some(n.value as u8);
            }
        }
        None
    }
    match expression.without_parentheses() {
        Expression::ConditionalExpression(c) => {
            Some(vec![byte(&c.consequent)?, byte(&c.alternate)?])
        }
        e => byte(e).map(|v| vec![v]),
    }
}
pub(super) fn prefix_be32(elements: &[String]) -> bool {
    use oxc_syntax::operator::BinaryOperator as B;
    if elements.len() != 4 {
        return false;
    }
    let mut seed = None;
    for (index, text) in elements.iter().enumerate() {
        let arena = Allocator::default();
        let Ok(expr) = Parser::new(&arena, text, SourceType::mjs()).parse_expression() else {
            return false;
        };
        let mut e = expr.without_parentheses();
        let mut masked = false;
        if let Expression::BinaryExpression(b) = e {
            if b.operator == B::BitwiseAnd
                && matches!(b.right.without_parentheses(),Expression::NumericLiteral(n) if n.value==255.)
            {
                masked = true;
                e = b.left.without_parentheses();
            }
        }
        if index > 0 && !masked {
            return false;
        }
        let id = if index == 3 {
            e
        } else {
            let Expression::BinaryExpression(b) = e else {
                return false;
            };
            if !matches!(b.operator, B::ShiftRight | B::ShiftRightZeroFill)
                || !matches!(b.right.without_parentheses(),Expression::NumericLiteral(n) if n.value==((3-index)*8) as f64)
            {
                return false;
            }
            if !masked && b.operator != B::ShiftRightZeroFill {
                return false;
            }
            b.left.without_parentheses()
        };
        let Expression::Identifier(id) = id else {
            return false;
        };
        if seed.as_ref().is_some_and(|s| s != id.name.as_str()) {
            return false;
        }
        seed = Some(id.name.to_string());
    }
    true
}
pub(super) fn compile_node(
    n: &GrammarNode,
    e: &EmissionContract,
    a: &AlgorithmReport,
    plan: &super::SourcePlan,
    source: &str,
    depth: usize,
) -> Result<Op, WireError> {
    if depth > 64 {
        return Err(unsupported("wire program depth exceeds 64"));
    }
    if let GrammarNode::Byte(w) | GrammarNode::Number(w) | GrammarNode::String(w) = n {
        if w.mask == MaskForm::Stream
            && !w
                .stream_factory
                .zip(a.stream.span)
                .is_some_and(|(binding, span)| {
                    span.start <= binding.start && binding.end <= span.end
                })
        {
            return Err(unsupported(
                "this leaf's concrete stream factory is not certified",
            ));
        }
    }
    let grammar_path = super::control::grammar_path(&e.grammar, n).ok_or(WireError::Evidence)?;
    Ok(match n {
        GrammarNode::Number(w) => {
            if a.integer.codec.is_none() {
                return Err(unsupported("integer writer is not certified"));
            }
            if !w
                .writer_factory
                .zip(a.integer.span)
                .is_some_and(|(b, s)| s.start <= b.start && b.end <= s.end)
                && !(w.writer_factory.is_none() && {
                    let concrete = super::algorithms::number_at_call(source, w);
                    concrete.codec == a.integer.codec && concrete.span == a.integer.span
                })
            {
                return Err(unsupported(
                    "this leaf's concrete number writer is not certified",
                ));
            }
            if w.xor_literal.is_some() {
                return Err(unsupported("number value XOR semantics are unresolved"));
            }
            let literal = number_literal(&w.value).map(f64::to_bits);
            let envelope = a
                .integer
                .span
                .and_then(|span| super::control::number_envelope(source, span));
            if literal.is_none() && !source_float_prefix(source, a) && envelope.is_none() {
                return Err(unsupported(
                    "number's alternate-branch prefix is not certified",
                ));
            }
            Op::Number {
                envelope,
                mask: mask(w, e)?,
                literal,
            }
        }
        GrammarNode::String(w) => {
            if a.string.codec.is_none() {
                return Err(unsupported(
                    "string writer and intern scope are not certified",
                ));
            }
            if !w
                .writer_factory
                .zip(a.string.span)
                .is_some_and(|(b, s)| s.start <= b.start && b.end <= s.end)
            {
                return Err(unsupported(
                    "this leaf's concrete string writer is not certified",
                ));
            }
            if w.xor_literal.is_some() {
                return Err(unsupported("string value XOR semantics are unresolved"));
            }
            let arena = Allocator::default();
            let expression = Parser::new(&arena, &w.value, SourceType::mjs())
                .parse_expression()
                .ok();
            let literal = match expression.as_ref().map(Expression::without_parentheses) {
                Some(Expression::StringLiteral(s)) => {
                    if s.lone_surrogates {
                        return Err(unsupported(
                            "surrogate source literal validation is unresolved",
                        ));
                    }
                    Some(s.value.encode_utf16().collect())
                }
                _ => None,
            };
            Op::String {
                mask: mask(w, e)?,
                literal,
            }
        }
        GrammarNode::Byte(w) => {
            let xor = match w.xor_literal {
                None => 0,
                Some(x)
                    if x.is_finite()
                        && x.fract() == 0.
                        && x >= i32::MIN as f64
                        && x <= u32::MAX as f64 =>
                {
                    x as i64 as u8
                }
                _ => return Err(unsupported("static byte XOR is unresolved")),
            };
            Op::Byte {
                mask: mask(w, e)?,
                xor,
                allowed: allowed(w),
            }
        }
        GrammarNode::Sequence { items } => Op::Sequence {
            items: items
                .iter()
                .map(|n| compile_node(n, e, a, plan, source, depth + 1))
                .collect::<Result<_, _>>()?,
        },
        GrammarNode::Repeat {
            count,
            direction,
            body,
            span,
            ..
        } => {
            if count.is_none() {
                let reference = plan
                    .counts
                    .iter()
                    .find(|r| {
                        r.repeat_path == grammar_path
                            && r.loop_span.start == span.start
                            && r.loop_span.end == span.end
                    })
                    .ok_or_else(|| {
                        unsupported(
                            "repeat needs a proven prior wire count; external bounds require input",
                        )
                    })?;
                return Ok(Op::CountedRepeat {
                    count: super::CountExpr::PriorNumber {
                        reference: reference.clone(),
                    },
                    direction: reference.direction,
                    body: Box::new(compile_node(body, e, a, plan, source, depth + 1)?),
                });
            }
            let count = count
                .and_then(|n| usize::try_from(n).ok())
                .filter(|n| *n <= 65536)
                .ok_or_else(|| {
                    unsupported(
                        "repeat needs a proven count (external loop bound is not a wire length)",
                    )
                })?;
            if matches!(direction, LoopDirection::Unknown) {
                return Err(unsupported("repeat direction is unresolved"));
            }
            Op::Repeat {
                count,
                direction: *direction,
                body: Box::new(compile_node(body, e, a, plan, source, depth + 1)?),
            }
        }
        GrammarNode::Choice { arms, span, .. } => {
            let arms: Vec<_> = arms
                .iter()
                .map(|arm| compile_node(&arm.body, e, a, plan, source, depth + 1))
                .collect::<Result<_, _>>()?;
            if let Some(proof) = plan.counter_choices.iter().find(|p| {
                p.path == grammar_path && p.choice.start == span.start && p.choice.end == span.end
            }) {
                return Ok(Op::CounterChoice {
                    proof: proof.clone(),
                    arms,
                });
            }
            fn domain(
                op: &Op,
                algorithms: &AlgorithmReport,
                source: &str,
            ) -> Option<(Vec<u8>, WireMask)> {
                match op {
                    Op::Byte {
                        allowed: Some(values),
                        xor,
                        mask,
                    } => Some((values.iter().map(|v| v ^ xor).collect(), mask.clone())),
                    Op::Number {
                        mask,
                        literal: Some(bits),
                        ..
                    } => {
                        let number = f64::from_bits(*bits);
                        if !number.is_finite() || number as i32 as f64 != number {
                            return None;
                        }
                        let magnitude = (number as i32).unsigned_abs();
                        let tag = if number.is_sign_negative() { 64 } else { 0 }
                            | (magnitude as u8 & 31)
                            | if magnitude >= 32 { 32 } else { 0 };
                        Some((vec![tag], mask.clone()))
                    }
                    // Certified NUMBER has a first byte <= its float marker.
                    // Unknown float byte order does not change this domain.
                    Op::Number {
                        mask,
                        envelope: Some(envelope),
                        ..
                    } => Some(((0..=envelope.first_byte_maximum).collect(), mask.clone())),
                    Op::Number {
                        mask,
                        literal: None,
                        ..
                    } if source_float_prefix(source, algorithms) => {
                        Some(((0..=128).collect(), mask.clone()))
                    }
                    // New modified UTF-8 strings, including zero terminator,
                    // plus the interner marker. Continuation bytes cannot lead.
                    Op::String { mask, .. } => Some((
                        (0..=127).chain([129, 192]).chain(194..=244).collect(),
                        mask.clone(),
                    )),
                    Op::Sequence { items } => domain(items.first()?, algorithms, source),
                    _ => None,
                }
            }
            let heads = arms
                .iter()
                .map(|op| domain(op, a, source))
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| {
                    unsupported(
                        "choice requires an external branch input; no proven wire discriminant",
                    )
                })?;
            let mask = heads
                .first()
                .ok_or_else(|| unsupported("empty choice"))?
                .1
                .clone();
            let mut seen = std::collections::BTreeSet::new();
            let mut domains = Vec::new();
            for (domain, m) in heads {
                if m != mask || domain.is_empty() || domain.iter().any(|b| !seen.insert(*b)) {
                    return Err(unsupported("choice has no unique common-mask discriminant"));
                }
                domains.push(domain);
            }
            Op::DomainChoice {
                source: (*span).into(),
                path: grammar_path.clone(),
                consumption: TagConsumption::ArmOwnsTag,
                semantics: plan
                    .choices
                    .iter()
                    .find(|p| {
                        p.path == grammar_path
                            && p.choice.start == span.start
                            && p.choice.end == span.end
                    })
                    .and_then(|p| p.semantics.clone()),
                domains,
                mask,
                arms,
            }
        }
        GrammarNode::RawPrefix(_) => {
            return Err(unsupported("nested prefix/stream reset is unresolved"))
        }
        GrammarNode::Opaque { .. } | GrammarNode::Unresolved { .. } => {
            return Err(unsupported(
                "contract contains an opaque or unresolved write",
            ))
        }
    })
}
fn number_literal(text: &str) -> Option<f64> {
    fn value(expression: &Expression<'_>) -> Option<f64> {
        match expression.without_parentheses() {
            Expression::NumericLiteral(n) => Some(n.value),
            Expression::UnaryExpression(u)
                if u.operator == oxc_syntax::operator::UnaryOperator::UnaryNegation =>
            {
                value(&u.argument).map(|n| -n)
            }
            _ => None,
        }
    }
    let arena = Allocator::default();
    let expression = Parser::new(&arena, text, SourceType::mjs())
        .parse_expression()
        .ok()?;
    value(&expression)
}
fn enabled(mask: &WireMask) -> Result<bool, WireError> {
    match mask {
        WireMask::Inherit => Ok(true),
        WireMask::None => Ok(false),
        _ => Err(unsupported("nested or unresolved mask")),
    }
}
impl Program {
    /// Compare executable count references independently of their source anchors.
    /// Both programs must already have passed exact source/proof validation.
    /// All other operations retain exact equality, including salt and domains.
    pub(super) fn same_execution(&self, other: &Self) -> bool {
        fn ops(a: &Op, b: &Op) -> bool {
            match (a, b) {
                (Op::Sequence { items: a }, Op::Sequence { items: b }) => {
                    a.len() == b.len() && a.iter().zip(b).all(|(a, b)| ops(a, b))
                }
                (
                    Op::CountedRepeat {
                        count: super::CountExpr::PriorNumber { reference: a },
                        direction: ad,
                        body: ab,
                    },
                    Op::CountedRepeat {
                        count: super::CountExpr::PriorNumber { reference: b },
                        direction: bd,
                        body: bb,
                    },
                ) => {
                    ad == bd
                        && a.path == b.path
                        && a.repeat_path == b.repeat_path
                        && a.direction == b.direction
                        && a.maximum == b.maximum
                        && a.reject_negative_zero == b.reject_negative_zero
                        && a.update_in_body == b.update_in_body
                        && a.predecrement_exit.is_some() == b.predecrement_exit.is_some()
                        && a.same_cached_evaluation == b.same_cached_evaluation
                        && ops(ab, bb)
                }
                _ => a == b,
            }
        }
        self.prefix_byte_order == other.prefix_byte_order
            && self.seed_source == other.seed_source
            && self.stream_scope == other.stream_scope
            && self.string_scope == other.string_scope
            && self.salt == other.salt
            && ops(&self.body, &other.body)
    }

    /// Conservative whole-message language bounds, including captured RAW4.
    /// An unknown upper bound is never used to eliminate a candidate.
    pub(super) fn length_bounds(&self) -> (usize, Option<usize>) {
        fn add(a: (usize, Option<usize>), b: (usize, Option<usize>)) -> (usize, Option<usize>) {
            (
                a.0.saturating_add(b.0),
                a.1.zip(b.1).and_then(|(a, b)| a.checked_add(b)),
            )
        }
        fn bounds(op: &Op) -> (usize, Option<usize>) {
            match op {
                Op::RootChoice { cases } => (
                    cases
                        .iter()
                        .map(|c| c.plan.minimum_length.saturating_sub(4))
                        .min()
                        .unwrap_or(0),
                    cases
                        .iter()
                        .map(|c| c.plan.maximum_length.and_then(|n| n.checked_sub(4)))
                        .collect::<Option<Vec<_>>>()
                        .and_then(|v| v.into_iter().max()),
                ),
                Op::Byte { .. } => (1, Some(1)),
                Op::Number { envelope, .. } => (1, envelope.as_ref().map(|e| e.maximum_bytes)),
                Op::String { .. } => (1, None),
                Op::Sequence { items } => {
                    items.iter().fold((0, Some(0)), |a, op| add(a, bounds(op)))
                }
                Op::Repeat { count, body, .. } => {
                    let (min, max) = bounds(body);
                    (
                        min.saturating_mul(*count),
                        max.and_then(|n| n.checked_mul(*count)),
                    )
                }
                // CountRef.maximum limits execution resources. It is not a
                // source upper bound and cannot disprove language membership.
                Op::CountedRepeat { .. } => (0, None),
                Op::Choice { arms, .. }
                | Op::DomainChoice { arms, .. }
                | Op::CounterChoice { arms, .. } => {
                    let arms: Vec<_> = arms.iter().map(bounds).collect();
                    (
                        arms.iter().map(|b| b.0).min().unwrap_or(0),
                        arms.iter()
                            .map(|b| b.1)
                            .collect::<Option<Vec<_>>>()
                            .and_then(|v| v.into_iter().max()),
                    )
                }
            }
        }
        add((4, Some(4)), bounds(&self.body))
    }
    pub(super) fn decode(&self, bytes: &[u8], token: i32) -> Result<WireValue, WireError> {
        self.decode_with_float(
            bytes,
            token,
            &Err(unsupported("float runtime byte order is unknown")),
        )
    }
    pub(super) fn decode_with_float(
        &self,
        bytes: &[u8],
        token: i32,
        format: &Result<super::FloatByteOrder, WireError>,
    ) -> Result<WireValue, WireError> {
        self.decode_source(bytes, token, format, None, &[])
            .map(|(v, _)| v)
    }
    pub(super) fn decode_source(
        &self,
        bytes: &[u8],
        token: i32,
        format: &Result<super::FloatByteOrder, WireError>,
        execution: Option<super::source_number::Execution>,
        prefix: &[usize],
    ) -> Result<(WireValue, super::source_number::Forms), WireError> {
        if let Op::RootChoice { cases } = &self.body {
            let candidates: Vec<_> = cases
                .iter()
                .filter(|c| {
                    bytes.len() >= c.plan.minimum_length
                        && c.plan.maximum_length.is_none_or(|m| bytes.len() <= m)
                })
                .collect();
            if candidates.len() > 1 {
                return Err(WireError::Ambiguous);
            }
            let case = candidates.first().ok_or(WireError::Discriminant)?;
            let program = match case.support.as_ref() {
                Support::Supported { program } => program,
                Support::Unsupported { reason, .. } => {
                    return Err(WireError::Unsupported(reason.clone()))
                }
                Support::RequiresInitializer { .. } => {
                    return Err(unsupported(
                        "selected root case requires its trusted initialization witness",
                    ))
                }
            };
            let (value, forms) = program.decode_source(
                bytes,
                token,
                format,
                execution,
                &child(prefix, case.plan.arm),
            )?;
            return Ok((
                WireValue::Choice {
                    arm: case.plan.arm,
                    value: Box::new(value),
                },
                forms,
            ));
        }
        let mut reader = SignalReader::new(bytes, token, self.salt).map_err(WireError::codec)?;
        let mut budget = Budget::new(format.clone());
        budget.source = execution;
        budget.source_prefix = prefix.to_vec();
        let value = read(&self.body, &mut reader, &mut budget, &[])?;
        if !reader.at_end() {
            return Err(WireError::Trailing(reader.remaining()));
        }
        let forms = budget
            .source
            .map(|s| s.finish())
            .transpose()?
            .unwrap_or_default();
        Ok((value, forms))
    }
    pub(super) fn encode(
        &self,
        value: &WireValue,
        seed: u32,
        token: i32,
    ) -> Result<Vec<u8>, WireError> {
        self.encode_with_float(
            value,
            seed,
            token,
            &Err(unsupported("float runtime byte order is unknown")),
        )
    }
    pub(super) fn encode_with_float(
        &self,
        value: &WireValue,
        seed: u32,
        token: i32,
        format: &Result<super::FloatByteOrder, WireError>,
    ) -> Result<Vec<u8>, WireError> {
        self.encode_source(value, seed, token, format, None, &[])
    }
    pub(super) fn encode_source(
        &self,
        value: &WireValue,
        seed: u32,
        token: i32,
        format: &Result<super::FloatByteOrder, WireError>,
        execution: Option<super::source_number::Execution>,
        prefix: &[usize],
    ) -> Result<Vec<u8>, WireError> {
        if let Op::RootChoice { cases } = &self.body {
            let WireValue::Choice { arm, value } = value else {
                return Err(WireError::Type);
            };
            let case = cases
                .iter()
                .find(|c| c.plan.arm == *arm)
                .ok_or(WireError::Discriminant)?;
            let program = match case.support.as_ref() {
                Support::Supported { program } => program,
                Support::Unsupported { reason, .. } => {
                    return Err(WireError::Unsupported(reason.clone()))
                }
                Support::RequiresInitializer { .. } => {
                    return Err(unsupported(
                        "selected root case requires its trusted initialization witness",
                    ))
                }
            };
            let bytes = program.encode_source(
                value,
                seed,
                token,
                format,
                execution.clone(),
                &child(prefix, *arm),
            )?;
            // Encoding cannot produce an implicitly selected ambiguous root.
            let fresh = execution.map(|e| e.reader());
            let (selected, _) = self.decode_source(&bytes, token, format, fresh, prefix)?;
            if selected
                != (WireValue::Choice {
                    arm: *arm,
                    value: value.clone(),
                })
            {
                return Err(WireError::Reencode);
            }
            return Ok(bytes);
        }
        let mut writer = SignalWriter::new(token, seed, self.salt);
        let mut budget = Budget::new(format.clone());
        budget.source = execution;
        budget.source_prefix = prefix.to_vec();
        write(&self.body, value, &mut writer, &mut budget, &[])?;
        if let Some(source) = budget.source {
            source.finish()?;
        }
        let bytes = writer.finish();
        if bytes.len() > crate::signals::payload::MAX_PLAINTEXT_BYTES {
            return Err(WireError::Bounds);
        }
        Ok(bytes)
    }
}
struct Budget {
    source: Option<super::source_number::Execution>,
    source_prefix: Vec<usize>,
    float: Result<super::FloatByteOrder, WireError>,
    steps: usize,
    units: usize,
    numbers: std::collections::BTreeMap<Vec<usize>, u64>,
    counters: std::collections::BTreeMap<super::control::ControlPath, u32>,
    ordinals: std::collections::BTreeMap<super::control::ControlPath, u32>,
}
impl Budget {
    fn new(float: Result<super::FloatByteOrder, WireError>) -> Self {
        Self {
            float,
            source: None,
            source_prefix: Vec::new(),
            steps: 65536,
            numbers: Default::default(),
            counters: Default::default(),
            ordinals: Default::default(),
            units: crate::signals::payload::MAX_PLAINTEXT_BYTES,
        }
    }
    fn units(&mut self, length: usize) -> Result<(), WireError> {
        self.units = self.units.checked_sub(length).ok_or(WireError::Bounds)?;
        Ok(())
    }
}
fn charge(budget: &mut Budget) -> Result<(), WireError> {
    if budget.steps == 0 {
        return Err(WireError::Bounds);
    }
    budget.steps -= 1;
    Ok(())
}
fn read(
    op: &Op,
    r: &mut SignalReader<'_>,
    budget: &mut Budget,
    path: &[usize],
) -> Result<WireValue, WireError> {
    charge(budget)?;
    Ok(match op {
        Op::RootChoice { .. } => {
            return Err(unsupported("root case operation occurs inside a body"))
        }
        Op::Byte { mask, xor, allowed } => {
            r.masking(enabled(mask)?);
            let b = r.byte().map_err(WireError::codec)? ^ xor;
            if allowed.as_ref().is_some_and(|a| !a.contains(&b)) {
                return Err(WireError::Literal);
            }
            WireValue::Byte(b)
        }
        Op::Number { mask, literal, .. } => {
            r.masking(enabled(mask)?);
            let n = match &mut budget.source {
                Some(source) if literal.is_none() => source.read(
                    r,
                    &[budget.source_prefix.as_slice(), path].concat(),
                    &budget.float,
                )?,
                _ => super::float::read(r, &budget.float)?,
            };
            if literal.is_some_and(|bits| bits != n.to_bits()) {
                return Err(WireError::Literal);
            }
            budget.numbers.insert(path.to_vec(), n.to_bits());
            WireValue::NumberBits(n.to_bits())
        }
        Op::String { mask, literal } => {
            r.masking(enabled(mask)?);
            let units = r.string_units().map_err(WireError::codec)?;
            budget.units(units.len())?;
            if literal.as_ref().is_some_and(|expected| *expected != units) {
                return Err(WireError::Literal);
            }
            WireValue::StringUnits(units)
        }
        Op::Sequence { items } => WireValue::Sequence(
            items
                .iter()
                .enumerate()
                .map(|(i, n)| read(n, r, budget, &child(path, i)))
                .collect::<Result<_, _>>()?,
        ),
        Op::Repeat { count, body, .. } => {
            let mut values = Vec::new();
            for index in 0..*count {
                if let Some(s) = &mut budget.source {
                    s.occurrences.push(index);
                }
                values.push(read(body, r, budget, &child(path, 0))?);
                if let Some(s) = &mut budget.source {
                    s.occurrences.pop();
                }
            }
            WireValue::Repeat(values)
        }
        Op::CountedRepeat {
            count,
            direction,
            body,
        } => {
            let length = prior_count(count, budget)?;
            let super::CountExpr::PriorNumber { reference } = count;
            let mut values = Vec::new();
            for index in 0..length {
                let counter = if *direction == LoopDirection::Descending {
                    length - 1 - index
                } else {
                    index
                };
                budget
                    .counters
                    .insert(reference.repeat_path.clone(), counter as u32);
                budget
                    .ordinals
                    .insert(reference.repeat_path.clone(), index as u32);
                if let Some(s) = &mut budget.source {
                    s.occurrences.push(index);
                }
                values.push(read(body, r, budget, &child(path, 0))?);
                if let Some(s) = &mut budget.source {
                    s.occurrences.pop();
                }
            }
            budget.counters.remove(&reference.repeat_path);
            budget.ordinals.remove(&reference.repeat_path);
            WireValue::Repeat(values)
        }
        Op::CounterChoice { proof, arms } => {
            let value = counter_value(proof, budget)?;
            let arm = usize::from(!proof.predicate.test(value));
            WireValue::Choice {
                arm,
                value: Box::new(read(&arms[arm], r, budget, &child(path, arm))?),
            }
        }
        Op::DomainChoice {
            domains,
            mask,
            arms,
            ..
        } => {
            r.masking(enabled(mask)?);
            let mut peek = r.clone();
            let tag = peek.byte().map_err(WireError::codec)?;
            let arm = domains
                .iter()
                .position(|d| d.contains(&tag))
                .ok_or(WireError::Discriminant)?;
            WireValue::Choice {
                arm,
                value: Box::new(read(&arms[arm], r, budget, &child(path, arm))?),
            }
        }
        Op::Choice {
            discriminants,
            mask,
            xor,
            arms,
        } => {
            r.masking(enabled(mask)?);
            let mut peek = r.clone();
            let tag = peek.byte().map_err(WireError::codec)? ^ xor;
            let arm = discriminants
                .iter()
                .position(|domain| domain.contains(&tag))
                .ok_or(WireError::Discriminant)?;
            WireValue::Choice {
                arm,
                value: Box::new(read(&arms[arm], r, budget, &child(path, arm))?),
            }
        }
    })
}
fn write(
    op: &Op,
    v: &WireValue,
    w: &mut SignalWriter,
    budget: &mut Budget,
    path: &[usize],
) -> Result<(), WireError> {
    charge(budget)?;
    match (op, v) {
        (Op::Byte { mask, xor, allowed }, WireValue::Byte(b)) => {
            if allowed.as_ref().is_some_and(|a| !a.contains(b)) {
                return Err(WireError::Literal);
            }
            w.masking(enabled(mask)?);
            w.byte(b ^ xor);
        }
        (Op::Number { mask, literal, .. }, WireValue::NumberBits(bits)) => {
            if literal.is_some_and(|expected| expected != *bits) {
                return Err(WireError::Literal);
            }
            w.masking(enabled(mask)?);
            match &mut budget.source {
                Some(source) if literal.is_none() => source.write(
                    w,
                    *bits,
                    &[budget.source_prefix.as_slice(), path].concat(),
                    &budget.float,
                )?,
                _ => super::float::write(w, *bits, &budget.float)?,
            }
            budget.numbers.insert(path.to_vec(), *bits);
        }
        (Op::String { mask, literal }, WireValue::StringUnits(s)) => {
            budget.units(s.len())?;
            if literal.as_ref().is_some_and(|expected| expected != s) {
                return Err(WireError::Literal);
            }
            w.masking(enabled(mask)?);
            w.string_units(s);
        }
        (Op::Sequence { items }, WireValue::Sequence(values)) if items.len() == values.len() => {
            for (i, (n, v)) in items.iter().zip(values).enumerate() {
                write(n, v, w, budget, &child(path, i))?;
            }
        }
        (Op::Repeat { count, body, .. }, WireValue::Repeat(values)) if *count == values.len() => {
            for (index, v) in values.iter().enumerate() {
                if let Some(s) = &mut budget.source {
                    s.occurrences.push(index);
                }
                write(body, v, w, budget, &child(path, 0))?;
                if let Some(s) = &mut budget.source {
                    s.occurrences.pop();
                }
            }
        }
        (
            Op::CountedRepeat {
                count,
                direction,
                body,
            },
            WireValue::Repeat(values),
        ) => {
            let length = prior_count(count, budget)?;
            if length != values.len() {
                return Err(WireError::Type);
            }
            let super::CountExpr::PriorNumber { reference } = count;
            for (index, v) in values.iter().enumerate() {
                let counter = if *direction == LoopDirection::Descending {
                    length - 1 - index
                } else {
                    index
                };
                budget
                    .counters
                    .insert(reference.repeat_path.clone(), counter as u32);
                budget
                    .ordinals
                    .insert(reference.repeat_path.clone(), index as u32);
                if let Some(s) = &mut budget.source {
                    s.occurrences.push(index);
                }
                write(body, v, w, budget, &child(path, 0))?;
                if let Some(s) = &mut budget.source {
                    s.occurrences.pop();
                }
            }
            budget.counters.remove(&reference.repeat_path);
            budget.ordinals.remove(&reference.repeat_path);
        }
        (Op::CounterChoice { proof, arms }, WireValue::Choice { arm, value }) => {
            let counter = counter_value(proof, budget)?;
            if *arm != usize::from(!proof.predicate.test(counter)) {
                return Err(WireError::Discriminant);
            }
            write(
                arms.get(*arm).ok_or(WireError::Discriminant)?,
                value,
                w,
                budget,
                &child(path, *arm),
            )?;
        }
        (
            Op::Choice { arms, .. } | Op::DomainChoice { arms, .. },
            WireValue::Choice { arm, value },
        ) => write(
            arms.get(*arm).ok_or(WireError::Discriminant)?,
            value,
            w,
            budget,
            &child(path, *arm),
        )?,
        _ => return Err(WireError::Type),
    }
    Ok(())
}

fn child(path: &[usize], i: usize) -> Vec<usize> {
    let mut p = path.to_vec();
    p.push(i);
    p
}
fn prior_count(expr: &super::CountExpr, budget: &Budget) -> Result<usize, WireError> {
    let super::CountExpr::PriorNumber { reference } = expr;
    let mut path = reference.path.clone();
    // Compilation removes exactly the root prefix, preserving every other
    // physical node and all source grammar paths in the certificate.
    let head = path.first_mut().ok_or(WireError::Evidence)?;
    *head = head.checked_sub(1).ok_or(WireError::Evidence)?;
    let n = f64::from_bits(*budget.numbers.get(&path).ok_or(WireError::Evidence)?);
    if !n.is_finite()
        || n < 0.
        || n.fract() != 0.
        || n > reference.maximum as f64
        || (reference.reject_negative_zero && n == 0. && n.is_sign_negative())
    {
        return Err(WireError::Bounds);
    }
    Ok(n as usize)
}

fn counter_value(
    proof: &super::control::CounterChoiceProof,
    budget: &Budget,
) -> Result<u32, WireError> {
    use super::control::{CounterInput, PackedPhase};
    match &proof.input {
        CounterInput::Induction => budget
            .counters
            .get(&proof.counter.repeat_path)
            .copied()
            .ok_or(WireError::Evidence),
        CounterInput::Packed {
            count,
            modulus,
            phase,
            ..
        } => {
            if *modulus != 8 {
                return Err(WireError::Evidence);
            }
            match phase {
                PackedPhase::BeforeFlush => budget
                    .ordinals
                    .get(&proof.counter.repeat_path)
                    .map(|i| i % modulus + 1)
                    .ok_or(WireError::Evidence),
                PackedPhase::AfterLoop => Ok(prior_count(
                    &super::CountExpr::PriorNumber {
                        reference: count.clone(),
                    },
                    budget,
                )? as u32
                    % modulus),
            }
        }
    }
}
