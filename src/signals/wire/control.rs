//! Static control-flow certificates. Expressions are joined by resolved source
//! bindings and reaching definitions, never by their printed spelling.
use crate::signals::{
    extract::contracts::{EmissionContract, GrammarNode, LoopDirection},
    schema::SourceSpan,
};
use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_parser::Parser;
use oxc_semantic::{Scoping, SymbolId};
use oxc_span::{GetSpan, SourceType, Span};
use oxc_syntax::operator::{AssignmentOperator, BinaryOperator, UpdateOperator};
use serde::{Deserialize, Serialize};

pub type ControlPath = Vec<ControlStep>;
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlStep {
    Child(usize),
    Arm(usize),
    Body,
}

/// Paths disambiguate distinct inlined occurrences of one helper span.
pub(super) fn grammar_path(root: &GrammarNode, needle: &GrammarNode) -> Option<ControlPath> {
    fn walk(root: &GrammarNode, needle: &GrammarNode, path: &mut ControlPath) -> bool {
        if std::ptr::eq(root, needle) {
            return true;
        }
        match root {
            GrammarNode::Sequence { items } => {
                for (i, n) in items.iter().enumerate() {
                    path.push(ControlStep::Child(i));
                    if walk(n, needle, path) {
                        return true;
                    }
                    path.pop();
                }
            }
            GrammarNode::Choice { arms, .. } => {
                for (i, a) in arms.iter().enumerate() {
                    path.push(ControlStep::Arm(i));
                    if walk(&a.body, needle, path) {
                        return true;
                    }
                    path.pop();
                }
            }
            GrammarNode::Repeat { body, .. } => {
                path.push(ControlStep::Body);
                if walk(body, needle, path) {
                    return true;
                }
                path.pop();
            }
            _ => {}
        }
        false
    }
    let mut path = Vec::new();
    walk(root, needle, &mut path).then_some(path)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CountRef {
    /// Original grammar child path, including the non-value RAW4 child.
    pub path: Vec<usize>,
    pub repeat_path: ControlPath,
    pub write: SourceSpan,
    pub collection_binding: SourceSpan,
    pub counter_binding: SourceSpan,
    pub update: SourceSpan,
    pub update_in_body: bool,
    pub predecrement_exit: Option<SourceSpan>,
    pub length_evaluation: SourceSpan,
    pub same_cached_evaluation: bool,
    pub loop_span: SourceSpan,
    pub direction: LoopDirection,
    pub maximum: usize,
    pub reject_negative_zero: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CountExpr {
    PriorNumber { reference: CountRef },
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourcePlan {
    pub counts: Vec<CountRef>,
    pub root_cases: Vec<RootCasePlan>,
    pub counter_choices: Vec<CounterChoiceProof>,
    pub choices: Vec<ChoiceDomainProof>,
    pub unproven_counts: Vec<UnprovenCount>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnprovenCount {
    pub path: ControlPath,
    pub loop_span: SourceSpan,
    pub reason: CountRefusal,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CountRefusal {
    Induction,
    ControlTransfer,
    Mutation,
    ReachingBound,
    PriorWireCount,
    StableLength,
    UnaccountedCall,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChoiceDomainProof {
    pub choice: SourceSpan,
    pub path: ControlPath,
    pub domains: Vec<Vec<u8>>,
    pub mask: super::WireMask,
    pub consumption: super::TagConsumption,
    pub semantics: Option<ChoiceSemantics>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ChoiceSemantics {
    Nullish {
        binding: SourceSpan,
        literal_arm: usize,
        guard_read: SourceSpan,
        value_read: SourceSpan,
    },
    NullOnly {
        binding: SourceSpan,
        literal_arm: usize,
        guard_read: SourceSpan,
        value_read: SourceSpan,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CounterRef {
    pub loop_span: SourceSpan,
    pub repeat_path: ControlPath,
    pub binding: SourceSpan,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BitPredicate {
    pub mask: u32,
    pub equals: u32,
    pub inverted: bool,
}
impl BitPredicate {
    pub(super) fn test(&self, value: u32) -> bool {
        ((value & self.mask) == self.equals) != self.inverted
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CounterChoiceProof {
    pub choice: SourceSpan,
    pub read: SourceSpan,
    pub path: ControlPath,
    pub counter: CounterRef,
    pub predicate: BitPredicate,
    pub input: CounterInput,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CounterInput {
    Induction,
    Packed {
        count: CountRef,
        increment: SourceSpan,
        reset: SourceSpan,
        modulus: u32,
        phase: PackedPhase,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackedPhase {
    BeforeFlush,
    AfterLoop,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RootCasePlan {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_choice: Option<SourceSpan>,
    pub arm: usize,
    pub prefix: SourceSpan,
    pub stream: SourceSpan,
    pub seed_binding: SourceSpan,
    pub salt: u32,
    pub minimum_length: usize,
    pub maximum_length: Option<usize>,
}

pub(super) fn root_emissions(
    source: &str,
    emission: &EmissionContract,
) -> Option<Vec<(RootCasePlan, EmissionContract)>> {
    use crate::signals::extract::contracts::{MaskForm, StreamContract};
    let GrammarNode::Choice {
        arms, span: choice, ..
    } = &emission.grammar
    else {
        return None;
    };
    let arena = Allocator::default();
    let parsed = Parser::new(&arena, source, SourceType::mjs()).parse();
    if !parsed.errors.is_empty() {
        return None;
    }
    let semantic = oxc_semantic::SemanticBuilder::new()
        .build(&parsed.program)
        .semantic;
    let mut index = Index::default();
    index.visit_program(&parsed.program);
    let mut out = Vec::new();
    for (arm, branch) in arms.iter().enumerate() {
        let GrammarNode::Sequence { items } = &branch.body else {
            return None;
        };
        let Some(GrammarNode::RawPrefix(prefix)) = items.first() else {
            return None;
        };
        if !super::program::prefix_be32(&prefix.elements) {
            return None;
        }
        let mut leaves = Vec::new();
        for node in &items[1..] {
            match node {
                GrammarNode::Byte(w) | GrammarNode::Number(w) | GrammarNode::String(w) => {
                    leaves.push(w)
                }
                _ => return None,
            }
        }
        let first = *leaves.first()?;
        let instance = first.stream_instance?;
        if leaves
            .iter()
            .any(|w| w.mask != MaskForm::Stream || w.stream_instance != Some(instance))
        {
            return None;
        }
        let call = index
            .calls
            .iter()
            .find(|c| c.span.start == instance.start && c.span.end == instance.end)?;
        if call.arguments.len() != 1 {
            return None;
        }
        let argument = call.arguments[0].as_expression()?;
        let Expression::BinaryExpression(xor) = argument.without_parentheses() else {
            return None;
        };
        if xor.operator != BinaryOperator::BitwiseXOR {
            return None;
        }
        let (seed, salt) = match (
            xor.left.without_parentheses(),
            xor.right.without_parentheses(),
        ) {
            (Expression::Identifier(seed), Expression::NumericLiteral(salt)) => (seed, salt),
            _ => return None,
        };
        let symbol = semantic
            .scoping()
            .get_reference(seed.reference_id.get()?)
            .symbol_id()?;
        let seed_binding: SourceSpan = semantic.scoping().symbol_span(symbol).into();
        if prefix.seed_binding != Some(seed_binding)
            || !salt.value.is_finite()
            || salt.value.fract() != 0.
            || salt.value < i32::MIN as f64
            || salt.value > u32::MAX as f64
        {
            return None;
        }
        let mut minimum = 4usize;
        let mut maximum = Some(4usize);
        for node in &items[1..] {
            minimum += 1;
            let size = match node {
                GrammarNode::Byte(_) => Some(1),
                GrammarNode::Number(w) => {
                    // The writer binding must be inside the exact function
                    // whose complete byte envelope is independently matched.
                    let binding = w.writer_factory?;
                    semantic.nodes().iter().find_map(|n| match n.kind() {
                        oxc_ast::AstKind::Function(f)
                            if contains(f.span, Span::new(binding.start, binding.end)) =>
                        {
                            if global_number_conversion(
                                semantic.scoping(),
                                semantic.nodes(),
                                f.span,
                            ) {
                                number_envelope(source, f.span.into()).map(|e| e.maximum_bytes)
                            } else {
                                None
                            }
                        }
                        _ => None,
                    })
                }
                GrammarNode::String(_) => None,
                _ => return None,
            };
            maximum = maximum.zip(size).and_then(|(a, b)| a.checked_add(b));
        }
        let plan = RootCasePlan {
            source_choice: Some((*choice).into()),
            arm,
            prefix: prefix.span.into(),
            stream: instance,
            seed_binding,
            salt: salt.value as i64 as u32,
            minimum_length: minimum,
            maximum_length: maximum,
        };
        let mut e = emission.clone();
        e.grammar = branch.body.clone();
        e.stream = Some(StreamContract {
            argument: source
                .get(argument.span().start as usize..argument.span().end as usize)?
                .into(),
            seed_word: None,
            salt: Some(salt.value.to_string()),
            salt_span: Some(salt.span.into()),
            instance: Some(instance),
            seed_binding: Some(seed_binding),
        });
        // The extractor's per-leaf argument must agree with the AST-derived
        // stream expression, independently of any global emission mask.
        if leaves
            .iter()
            .any(|w| w.mask_source.as_ref() != e.stream.as_ref().map(|s| &s.argument))
        {
            return None;
        }
        out.push((plan, e));
    }
    (!out.is_empty()).then_some(out)
}

/// This proves the byte envelope only. It does not choose a float path or
/// certify the byte order of a host-native typed array.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NumberEnvelope {
    pub source: SourceSpan,
    pub first_byte_maximum: u8,
    pub maximum_bytes: usize,
    pub proof: EnvelopeProof,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum EnvelopeProof {
    ReviewedReference,
    FixedWrites {
        marker: SourceSpan,
        tail: SourceSpan,
        position_binding: SourceSpan,
    },
}
fn global_number_conversion(
    scopes: &Scoping,
    nodes: &oxc_semantic::AstNodes<'_>,
    function: Span,
) -> bool {
    nodes.iter().all(|n| match n.kind() {
        oxc_ast::AstKind::IdentifierReference(i) if i.name == "Number" => {
            let Some(r) = i.reference_id.get().map(|r| scopes.get_reference(r)) else {
                return false;
            };
            (!contains(function, i.span) || r.symbol_id().is_none())
                && !(r.symbol_id().is_none() && r.is_write())
        }
        _ => true,
    })
}
pub(super) fn number_envelope(source: &str, span: SourceSpan) -> Option<NumberEnvelope> {
    let text = source.get(span.start as usize..span.end as usize)?;
    let proof = if super::algorithms::proof(
        Some((span, text)),
        include_str!("reference/number.js"),
        "number-byte-envelope/1",
        false,
    )
    .codec
    .is_some()
    {
        EnvelopeProof::ReviewedReference
    } else {
        fixed_number_writes(text, span.start)?
    };
    Some(NumberEnvelope {
        source: span,
        first_byte_maximum: 128,
        maximum_bytes: 9,
        proof,
    })
}

/// Certify only the NUMBER byte envelope around opaque float arithmetic.
/// This recognizes the marker, isolated computation and eight-byte emission
/// loop. It neither evaluates that arithmetic nor certifies float byte order.
fn fixed_number_writes(text: &str, offset: u32) -> Option<EnvelopeProof> {
    let whole = SourceSpan::new(0, text.len() as u32);
    super::algorithms::proof(
        Some((whole, text)),
        include_str!("reference/number.js"),
        "integer-envelope-prerequisite/1",
        true,
    )
    .codec?;
    let arena = Allocator::default();
    let parsed = Parser::new(&arena, text, SourceType::mjs()).parse();
    if !parsed.errors.is_empty() {
        return None;
    }
    let semantic = oxc_semantic::SemanticBuilder::new()
        .build(&parsed.program)
        .semantic;
    let scopes = semantic.scoping();
    let symbol = |e: &Expression<'_>| match e.without_parentheses() {
        Expression::Identifier(i) => i
            .reference_id
            .get()
            .and_then(|r| scopes.get_reference(r).symbol_id()),
        _ => None,
    };
    let Statement::FunctionDeclaration(function) = parsed.program.body.first()? else {
        return None;
    };
    if function.params.items.len() != 4 {
        return None;
    }
    let param = |i: usize| {
        function.params.items[i]
            .pattern
            .get_binding_identifier()?
            .symbol_id
            .get()
    };
    let mask = param(0)?;
    let output = param(2)?;
    let input_position = param(3)?;
    let statements = &function.body.as_ref()?.statements;
    let Statement::ReturnStatement(returned) = statements.last()? else {
        return None;
    };
    let position = symbol(returned.argument.as_ref()?)?;
    let branch = statements.iter().find_map(|s| match s {
        Statement::IfStatement(i) => Some(i),
        _ => None,
    })?;
    let Statement::BlockStatement(alternate) = branch.alternate.as_ref()? else {
        return None;
    };
    let items = &alternate.body;
    if items.len() < 3 {
        return None;
    }
    fn assignment<'a, 'b>(s: &'b Statement<'a>) -> Option<&'b AssignmentExpression<'a>> {
        match s {
            Statement::ExpressionStatement(e) => match e.expression.without_parentheses() {
                Expression::AssignmentExpression(a) if a.operator == AssignmentOperator::Assign => {
                    Some(a.as_ref())
                }
                _ => None,
            },
            _ => None,
        }
    }
    let first = assignment(&items[0])?;
    let AssignmentTarget::AssignmentTargetIdentifier(target) = &first.left else {
        return None;
    };
    if target
        .reference_id
        .get()
        .and_then(|r| scopes.get_reference(r).symbol_id())
        != Some(position)
    {
        return None;
    }
    let Expression::BinaryExpression(add) = first.right.without_parentheses() else {
        return None;
    };
    let literal = |e: &Expression<'_>, value: f64| matches!(e.without_parentheses(),Expression::NumericLiteral(n)if n.value==value);
    if add.operator != BinaryOperator::Addition || !literal(&add.right, 1.) {
        return None;
    }
    let Expression::CallExpression(convert) = add.left.without_parentheses() else {
        return None;
    };
    if convert.optional
        || convert.arguments.len() != 1
        || symbol(convert.arguments[0].as_expression()?) != Some(input_position)
        || !matches!(convert.callee.without_parentheses(),Expression::Identifier(i) if i.name=="Number" && i.reference_id.get().is_some_and(|r|scopes.get_reference(r).symbol_id().is_none()))
    {
        return None;
    }
    let marker = assignment(&items[1])?;
    let AssignmentTarget::ComputedMemberExpression(store) = &marker.left else {
        return None;
    };
    if symbol(&store.object) != Some(output) || symbol(&store.expression) != Some(position) {
        return None;
    }
    let Expression::BinaryExpression(xor) = marker.right.without_parentheses() else {
        return None;
    };
    if xor.operator != BinaryOperator::BitwiseXOR {
        return None;
    }
    let is_mask = |e: &Expression<'_>| matches!(e.without_parentheses(),Expression::CallExpression(c)if !c.optional&&c.arguments.is_empty()&&symbol(&c.callee)==Some(mask));
    if !((literal(&xor.left, 128.) && is_mask(&xor.right))
        || (is_mask(&xor.left) && literal(&xor.right, 128.)))
    {
        return None;
    }
    let Statement::ForStatement(tail) = items.last()? else {
        return None;
    };
    let Expression::BinaryExpression(test) = tail.test.as_ref()?.without_parentheses() else {
        return None;
    };
    let index = symbol(&test.left)?;
    if test.operator != BinaryOperator::LessThan || !literal(&test.right, 8.) {
        return None;
    }
    let initial = match tail.init.as_ref() {
        Some(ForStatementInit::AssignmentExpression(a))
            if a.operator == AssignmentOperator::Assign =>
        {
            let AssignmentTarget::AssignmentTargetIdentifier(i) = &a.left else {
                return None;
            };
            if i.reference_id
                .get()
                .and_then(|r| scopes.get_reference(r).symbol_id())
                != Some(index)
            {
                return None;
            }
            &a.right
        }
        Some(ForStatementInit::VariableDeclaration(d)) if d.declarations.len() == 1 => {
            let d = &d.declarations[0];
            if d.id.get_binding_identifier()?.symbol_id.get() != Some(index) {
                return None;
            }
            d.init.as_ref()?
        }
        None => match &items[items.len() - 2] {
            Statement::VariableDeclaration(d) if d.declarations.len() == 1 => {
                let d = &d.declarations[0];
                if d.id.get_binding_identifier()?.symbol_id.get() != Some(index) {
                    return None;
                }
                d.init.as_ref()?
            }
            s => {
                let a = assignment(s)?;
                let AssignmentTarget::AssignmentTargetIdentifier(i) = &a.left else {
                    return None;
                };
                if i.reference_id
                    .get()
                    .and_then(|r| scopes.get_reference(r).symbol_id())
                    != Some(index)
                {
                    return None;
                }
                &a.right
            }
        },
        _ => return None,
    };
    if !literal(initial, 0.) {
        return None;
    }
    let Expression::UpdateExpression(update) = tail.update.as_ref()?.without_parentheses() else {
        return None;
    };
    let SimpleAssignmentTarget::AssignmentTargetIdentifier(i) = &update.argument else {
        return None;
    };
    if update.operator != UpdateOperator::Increment
        || i.reference_id
            .get()
            .and_then(|r| scopes.get_reference(r).symbol_id())
            != Some(index)
    {
        return None;
    }
    let body = match &tail.body {
        Statement::BlockStatement(b) if b.body.len() == 1 => &b.body[0],
        s => s,
    };
    let write = assignment(body)?;
    let AssignmentTarget::ComputedMemberExpression(store_tail) = &write.left else {
        return None;
    };
    if symbol(&store_tail.object) != Some(output) {
        return None;
    }
    let Expression::UpdateExpression(advance) = store_tail.expression.without_parentheses() else {
        return None;
    };
    let SimpleAssignmentTarget::AssignmentTargetIdentifier(i) = &advance.argument else {
        return None;
    };
    if !advance.prefix
        || advance.operator != UpdateOperator::Increment
        || i.reference_id
            .get()
            .and_then(|r| scopes.get_reference(r).symbol_id())
            != Some(position)
    {
        return None;
    }
    let Expression::BinaryExpression(value) = write.right.without_parentheses() else {
        return None;
    };
    if value.operator != BinaryOperator::BitwiseXOR {
        return None;
    }
    let bytes = if is_mask(&value.left) {
        &value.right
    } else if is_mask(&value.right) {
        &value.left
    } else {
        return None;
    };
    let Expression::ComputedMemberExpression(bytes) = bytes.without_parentheses() else {
        return None;
    };
    if symbol(&bytes.expression) != Some(index) || symbol(&bytes.object).is_none() {
        return None;
    }
    let mut scan = Index::default();
    scan.visit_program(&parsed.program);
    if scan.dynamic.iter().any(|s| contains(alternate.span, *s))
        || scan.transfers.iter().any(|s| contains(alternate.span, *s))
    {
        return None;
    }
    let allowed = |s: SymbolId, spans: &[Span]| {
        scopes
            .get_resolved_references(s)
            .filter(|r| {
                contains(
                    alternate.span,
                    semantic.nodes().get_node(r.node_id()).span(),
                )
            })
            .all(|r| {
                let span = semantic.nodes().get_node(r.node_id()).span();
                spans.iter().any(|s| contains(*s, span))
            })
    };
    // No aliases, cursor resets, discarded stream draws, or hidden writes in
    // the opaque computation may change the envelope or its returned position.
    if !allowed(output, &[store.object.span(), store_tail.object.span()])
        || !allowed(mask, &[marker.right.span(), write.right.span()])
        || !allowed(
            position,
            &[
                target.span,
                store.expression.span(),
                advance.argument.span(),
            ],
        )
        || scopes.get_resolved_references(index).any(|r| {
            if !r.is_write() {
                return false;
            }
            let span = semantic.nodes().get_node(r.node_id()).span();
            let owner = scan
                .functions
                .iter()
                .copied()
                .filter(|f| contains(*f, span))
                .min_by_key(|f| f.size());
            owner != Some(function.body.as_ref().unwrap().span)
                || (contains(tail.span, span)
                    && !contains(update.span, span)
                    && !tail.init.as_ref().is_some_and(|i| contains(i.span(), span)))
        })
    {
        return None;
    }
    let absolute = |s: Span| SourceSpan::new(offset + s.start, offset + s.end);
    Some(EnvelopeProof::FixedWrites {
        marker: absolute(marker.span),
        tail: absolute(tail.span),
        position_binding: absolute(scopes.symbol_span(position)),
    })
}

fn contains(a: Span, b: Span) -> bool {
    a.start <= b.start && b.end <= a.end
}

#[derive(Default)]
struct Index<'a> {
    expressions: Vec<&'a Expression<'a>>,
    loops: Vec<Loop<'a>>,
    updates: Vec<&'a UpdateExpression<'a>>,
    definitions: Vec<(SymbolId, Span, &'a Expression<'a>)>,
    blocks: Vec<Span>,
    guards: Vec<Span>,
    calls: Vec<&'a CallExpression<'a>>,
    transfers: Vec<Span>,
    functions: Vec<Span>,
    dynamic: Vec<Span>,
}
struct Loop<'a> {
    span: Span,
    init: Option<&'a ForStatementInit<'a>>,
    test: Option<&'a Expression<'a>>,
    update: Option<&'a Expression<'a>>,
    body: &'a Statement<'a>,
}
impl<'a> Visit<'a> for Index<'a> {
    fn visit_break_statement(&mut self, n: &BreakStatement<'a>) {
        self.transfers.push(n.span);
    }
    fn visit_continue_statement(&mut self, n: &ContinueStatement<'a>) {
        self.transfers.push(n.span);
    }
    fn visit_return_statement(&mut self, n: &ReturnStatement<'a>) {
        self.transfers.push(n.span);
        walk::walk_return_statement(self, n);
    }
    fn visit_throw_statement(&mut self, n: &ThrowStatement<'a>) {
        self.transfers.push(n.span);
        walk::walk_throw_statement(self, n);
    }
    fn visit_expression(&mut self, e: &Expression<'a>) {
        let e = self.alloc(e);
        self.expressions.push(e);
        walk::walk_expression(self, e);
    }
    fn visit_for_statement(&mut self, n: &ForStatement<'a>) {
        let n = self.alloc(n);
        self.loops.push(Loop {
            span: n.span,
            init: n.init.as_ref(),
            test: n.test.as_ref(),
            update: n.update.as_ref(),
            body: &n.body,
        });
        walk::walk_for_statement(self, n);
    }
    fn visit_while_statement(&mut self, n: &WhileStatement<'a>) {
        let n = self.alloc(n);
        self.loops.push(Loop {
            span: n.span,
            init: None,
            test: Some(&n.test),
            update: None,
            body: &n.body,
        });
        walk::walk_while_statement(self, n);
    }
    fn visit_update_expression(&mut self, n: &UpdateExpression<'a>) {
        self.updates.push(self.alloc(n));
        walk::walk_update_expression(self, n);
    }
    fn visit_logical_expression(&mut self, n: &LogicalExpression<'a>) {
        self.guards.push(n.right.span());
        walk::walk_logical_expression(self, n);
    }
    fn visit_try_statement(&mut self, n: &TryStatement<'a>) {
        self.guards.push(n.span);
        walk::walk_try_statement(self, n);
    }
    fn visit_switch_case(&mut self, n: &SwitchCase<'a>) {
        self.guards.push(n.span);
        walk::walk_switch_case(self, n);
    }
    fn visit_function_body(&mut self, n: &FunctionBody<'a>) {
        self.functions.push(n.span);
        self.blocks.push(n.span);
        walk::walk_function_body(self, n);
    }
    fn visit_block_statement(&mut self, n: &BlockStatement<'a>) {
        let n = self.alloc(n);
        self.blocks.push(n.span);
        walk::walk_block_statement(self, n);
    }
    fn visit_if_statement(&mut self, n: &IfStatement<'a>) {
        let n = self.alloc(n);
        self.guards.push(n.consequent.span());
        if let Some(a) = &n.alternate {
            self.guards.push(a.span());
        }
        walk::walk_if_statement(self, n);
    }
    fn visit_conditional_expression(&mut self, n: &ConditionalExpression<'a>) {
        let n = self.alloc(n);
        self.guards
            .extend([n.consequent.span(), n.alternate.span()]);
        walk::walk_conditional_expression(self, n);
    }
    fn visit_variable_declarator(&mut self, n: &VariableDeclarator<'a>) {
        let n = self.alloc(n);
        if let (BindingPattern::BindingIdentifier(id), Some(value)) = (&n.id, &n.init) {
            if let Some(symbol) = id.symbol_id.get() {
                self.definitions.push((symbol, n.span, value));
            }
        }
        walk::walk_variable_declarator(self, n);
    }
    fn visit_call_expression(&mut self, n: &CallExpression<'a>) {
        let n = self.alloc(n);
        if matches!(n.callee.without_parentheses(), Expression::Identifier(id) if id.name == "eval")
        {
            self.dynamic.push(n.span);
        }
        self.calls.push(n);
        walk::walk_call_expression(self, n);
    }
    fn visit_with_statement(&mut self, n: &WithStatement<'a>) {
        self.dynamic.push(n.span);
        walk::walk_with_statement(self, n);
    }
}

struct Proof<'s, 'a> {
    index: Index<'a>,
    scopes: &'s Scoping,
    nodes: &'s oxc_semantic::AstNodes<'a>,
}
impl<'a> Proof<'_, 'a> {
    fn stable_reads(&self, symbol: SymbolId, from: Span, to: Span) -> bool {
        if from.end > to.start {
            return false;
        }
        let owner = |span| {
            self.index
                .functions
                .iter()
                .copied()
                .filter(|f| contains(*f, span))
                .min_by_key(|f| f.size())
        };
        let Some(function) = owner(from) else {
            return false;
        };
        if owner(to) != Some(function)
            || self
                .index
                .dynamic
                .iter()
                .any(|s| contains(function, *s) || contains(*s, from))
        {
            return false;
        }
        let intervening = |end| from.end < end && end <= to.start;
        if self
            .index
            .definitions
            .iter()
            .any(|(s, span, _)| *s == symbol && intervening(span.end))
        {
            return false;
        }
        !self.scopes.get_resolved_references(symbol).any(|r| {
            if !r.is_write() {
                return false;
            }
            let span = self.nodes.get_node(r.node_id()).span();
            if owner(span) != Some(function) {
                return true;
            }
            // Assignment targets are evaluated before the RHS, but the stored
            // value changes after it. Compare commit points, not target offsets.
            let mut node = r.node_id();
            loop {
                match self.nodes.kind(node) {
                    oxc_ast::AstKind::AssignmentExpression(a) if contains(a.left.span(), span) => {
                        return intervening(a.span.end)
                    }
                    oxc_ast::AstKind::UpdateExpression(u) => return intervening(u.span.end),
                    _ => {}
                }
                let parent = self.nodes.parent_id(node);
                if parent == node {
                    return true;
                }
                node = parent;
            }
        })
    }
    fn symbol(&self, e: &Expression<'a>) -> Option<SymbolId> {
        let Expression::Identifier(id) = e.without_parentheses() else {
            return None;
        };
        self.scopes
            .get_reference(id.reference_id.get()?)
            .symbol_id()
    }
    fn block(&self, span: Span) -> Option<Span> {
        self.index
            .blocks
            .iter()
            .copied()
            .filter(|b| contains(*b, span))
            .min_by_key(|b| b.size())
    }
    fn definition(&self, symbol: SymbolId, point: Span) -> Option<&'a Expression<'a>> {
        let (_, span, value) = self
            .index
            .definitions
            .iter()
            .filter(|(s, d, _)| *s == symbol && d.end <= point.start)
            .max_by_key(|(_, d, _)| d.end)?;
        if self.block(*span) != self.block(point)
            || self
                .index
                .loops
                .iter()
                .any(|n| contains(n.span, *span) && !contains(n.span, point))
            || self
                .index
                .guards
                .iter()
                .any(|g| contains(*g, *span) && !contains(*g, point))
            || self.scopes.get_resolved_references(symbol).any(|r| {
                r.is_write() && {
                    let s = self.nodes.get_node(r.node_id()).span();
                    s.start >= span.end && s.start < point.end
                }
            })
        {
            return None;
        }
        Some(value)
    }
    fn length(&self, e: &Expression<'a>, point: Span, depth: usize) -> Option<(SymbolId, Span)> {
        if depth > 8 {
            return None;
        }
        match e.without_parentheses() {
            Expression::StaticMemberExpression(m) if m.property.name == "length" && !m.optional => {
                self.symbol(&m.object).map(|s| (s, e.span()))
            }
            e => self.length(
                crate::signals::extract::reaching::expression_value_at_use(
                    self.scopes,
                    self.nodes,
                    e,
                )
                .ok()?
                .value,
                point,
                depth + 1,
            ),
        }
    }
    fn packed_choices(
        &self,
        count: &CountRef,
        emission: &EmissionContract,
    ) -> Vec<CounterChoiceProof> {
        let mut out = Vec::new();
        let loop_span = Span::new(count.loop_span.start, count.loop_span.end);
        let Some(loop_node) = self.index.loops.iter().find(|n| n.span == loop_span) else {
            return out;
        };
        let mut choices = Vec::new();
        emission.grammar.walk(&mut |g| {
            if let GrammarNode::Choice {
                kind: crate::signals::extract::contracts::ChoiceKind::If,
                span,
                arms,
                ..
            } = g
            {
                if arms.len() == 2
                    && arms[0].label == "then"
                    && arms[1].label == "else"
                    && matches!(arms[0].body, GrammarNode::Byte(_))
                    && arms[1].body.is_empty()
                {
                    choices.push((*span, grammar_path(&emission.grammar, g).unwrap()));
                }
            }
        });
        let zero = |e: &Expression<'_>| matches!(e.without_parentheses(), Expression::NumericLiteral(n) if n.value == 0.);
        let mut body_path = count.repeat_path.clone();
        body_path.push(ControlStep::Body);
        for (span, path) in choices
            .iter()
            .cloned()
            .filter(|(s, p)| contains(loop_span, *s) && p.starts_with(&body_path))
        {
            let Some(guard) = self.nodes.iter().find_map(|n| match n.kind() {
                oxc_ast::AstKind::IfStatement(n) if n.span == span => Some(n),
                _ => None,
            }) else {
                continue;
            };
            let Expression::BinaryExpression(test) = guard.test.without_parentheses() else {
                continue;
            };
            // An explicit no-write else can update unrelated local state. The
            // byte/empty grammar above and resolved counter-write checks below
            // still require exactly one unconditional increment and then reset;
            // an alternate that changes that recurrence cannot be certified.
            if test.operator != BinaryOperator::StrictEquality {
                continue;
            }
            let eight = |e: &Expression<'_>| matches!(e.without_parentheses(), Expression::NumericLiteral(n) if n.value == 8.);
            let symbol = if eight(&test.left) {
                self.symbol(&test.right)
            } else if eight(&test.right) {
                self.symbol(&test.left)
            } else {
                None
            };
            let Some(symbol) = symbol else { continue };
            let Some(initial) =
                self.definition(symbol, Span::new(loop_span.start, loop_span.start))
            else {
                continue;
            };
            if !zero(initial) {
                continue;
            }
            let updates: Vec<_> = self.index.updates.iter().copied().filter(|u| contains(loop_node.body.span(), u.span)
                && matches!(&u.argument, SimpleAssignmentTarget::AssignmentTargetIdentifier(id) if id.reference_id.get().and_then(|r| self.scopes.get_reference(r).symbol_id()) == Some(symbol))).collect();
            if updates.len() != 1 {
                continue;
            }
            let update = updates[0];
            if update.operator != UpdateOperator::Increment
                || update.span.end > guard.span.start
                || self
                    .index
                    .guards
                    .iter()
                    .any(|g| contains(*g, update.span) && !contains(*g, loop_span))
                || self
                    .index
                    .loops
                    .iter()
                    .any(|l| l.span != loop_span && contains(l.span, update.span))
            {
                continue;
            }
            let mut resets: Vec<_> = self
                .index
                .expressions
                .iter()
                .filter_map(|e| {
                    let Expression::AssignmentExpression(a) = e.without_parentheses() else {
                        return None;
                    };
                    let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left else {
                        return None;
                    };
                    (a.operator == AssignmentOperator::Assign
                        && zero(&a.right)
                        && contains(guard.consequent.span(), a.span)
                        && id
                            .reference_id
                            .get()
                            .and_then(|r| self.scopes.get_reference(r).symbol_id())
                            == Some(symbol))
                    .then_some(a.span)
                })
                .collect();
            resets.sort_unstable_by_key(|s| (s.start, s.end));
            resets.dedup();
            if resets.len() != 1 {
                continue;
            }
            let reset = resets[0];
            if self.index.guards.iter().any(|g| {
                contains(*g, reset) && *g != guard.consequent.span() && !contains(*g, loop_span)
            }) {
                continue;
            }
            let owner = |s| {
                self.index
                    .functions
                    .iter()
                    .copied()
                    .filter(|f| contains(*f, s))
                    .min_by_key(|f| f.size())
            };
            if self.scopes.get_resolved_references(symbol).any(|r| {
                let s = self.nodes.get_node(r.node_id()).span();
                owner(s) != owner(loop_span)
                    || (r.is_write()
                        && contains(loop_span, s)
                        && !contains(update.span, s)
                        && !contains(reset, s))
            }) {
                continue;
            }
            let counter = CounterRef {
                loop_span: count.loop_span,
                repeat_path: count.repeat_path.clone(),
                binding: self.scopes.symbol_span(symbol).into(),
            };
            let input = |phase| CounterInput::Packed {
                count: count.clone(),
                increment: update.span.into(),
                reset: reset.into(),
                modulus: 8,
                phase,
            };
            out.push(CounterChoiceProof {
                choice: span.into(),
                read: guard.test.span().into(),
                path: path.clone(),
                counter: counter.clone(),
                predicate: BitPredicate {
                    mask: u32::MAX,
                    equals: 8,
                    inverted: false,
                },
                input: input(PackedPhase::BeforeFlush),
            });
            // The final partial flush reads the recurrence's exit residue,
            // not a guessed emitted-byte count or a reversed byte array.
            for (tail, tail_path) in choices
                .iter()
                .cloned()
                .filter(|(s, p)| s.start >= loop_span.end && following(&count.repeat_path, p))
            {
                let Some(test) = self.nodes.iter().find_map(|n| match n.kind() {
                    oxc_ast::AstKind::IfStatement(n) if n.span == tail => Some(&n.test),
                    _ => None,
                }) else {
                    continue;
                };
                if self.block(tail) != self.block(loop_span)
                    || self.scopes.get_resolved_references(symbol).any(|r| {
                        r.is_write() && {
                            let s = self.nodes.get_node(r.node_id()).span();
                            // The same source binding may hold a byte position
                            // inside either arm AFTER its residue was read.
                            s.start >= loop_span.end && s.start < test.span().end
                        }
                    })
                {
                    continue;
                }
                let nonzero = self.symbol(test) == Some(symbol)
                    || match test.without_parentheses() {
                        Expression::BinaryExpression(b)
                            if b.operator == BinaryOperator::StrictInequality =>
                        {
                            (self.symbol(&b.left) == Some(symbol) && zero(&b.right))
                                || (zero(&b.left) && self.symbol(&b.right) == Some(symbol))
                        }
                        _ => false,
                    };
                if nonzero {
                    out.push(CounterChoiceProof {
                        choice: tail.into(),
                        read: test.span().into(),
                        path: tail_path.clone(),
                        counter: counter.clone(),
                        predicate: BitPredicate {
                            mask: u32::MAX,
                            equals: 0,
                            inverted: true,
                        },
                        input: input(PackedPhase::AfterLoop),
                    });
                }
            }
        }
        out
    }
    fn prove(
        &self,
        loop_span: Span,
        prior: &[(Vec<usize>, &crate::signals::extract::contracts::WriteNode)],
        emission: &EmissionContract,
        refusal: &mut CountRefusal,
    ) -> Option<CountRef> {
        let n = self.index.loops.iter().find(|n| n.span == loop_span)?;
        *refusal = CountRefusal::Induction;
        let (counter, update, bound, direction, exit) = if n.test.is_none()
            && n.init.is_none()
            && n.update.is_none()
        {
            let Statement::BlockStatement(body) = n.body else {
                return None;
            };
            let [Statement::ExpressionStatement(first), Statement::IfStatement(guard), ..] =
                body.body.as_slice()
            else {
                return None;
            };
            let Expression::UpdateExpression(update) = first.expression.without_parentheses()
            else {
                return None;
            };
            if update.operator != UpdateOperator::Decrement || guard.alternate.is_some() {
                return None;
            }
            let SimpleAssignmentTarget::AssignmentTargetIdentifier(id) = &update.argument else {
                return None;
            };
            let counter = self
                .scopes
                .get_reference(id.reference_id.get()?)
                .symbol_id()?;
            let (test, inverted) = match guard.test.without_parentheses() {
                Expression::BinaryExpression(b) => (b.as_ref(), false),
                Expression::UnaryExpression(u)
                    if u.operator == oxc_syntax::operator::UnaryOperator::LogicalNot =>
                {
                    match u.argument.without_parentheses() {
                        Expression::BinaryExpression(b) => (b.as_ref(), true),
                        _ => return None,
                    }
                }
                _ => return None,
            };
            let zero = |e: &Expression<'_>| matches!(e.without_parentheses(),Expression::NumericLiteral(n) if n.value==0.);
            let negative = if inverted {
                (test.operator == BinaryOperator::GreaterEqualThan
                    && self.symbol(&test.left) == Some(counter)
                    && zero(&test.right))
                    || (test.operator == BinaryOperator::LessEqualThan
                        && zero(&test.left)
                        && self.symbol(&test.right) == Some(counter))
            } else {
                (test.operator == BinaryOperator::LessThan
                    && self.symbol(&test.left) == Some(counter)
                    && zero(&test.right))
                    || (test.operator == BinaryOperator::GreaterThan
                        && zero(&test.left)
                        && self.symbol(&test.right) == Some(counter))
            };
            if !negative {
                return None;
            }
            let transfer = match &guard.consequent {
                Statement::BreakStatement(b) if b.label.is_none() => b.span,
                Statement::BlockStatement(b) => match b.body.as_slice() {
                    [Statement::BreakStatement(b)] if b.label.is_none() => b.span,
                    _ => return None,
                },
                _ => return None,
            };
            let start = self.definition(counter, Span::new(n.span.start, n.span.start))?;
            (
                counter,
                update.as_ref(),
                start,
                LoopDirection::Descending,
                Some(transfer),
            )
        } else {
            let Expression::BinaryExpression(test) = n.test?.without_parentheses() else {
                return None;
            };
            let counter = self.symbol(&test.left)?;
            let start = match n.init {
                Some(ForStatementInit::VariableDeclaration(d)) if d.declarations.len() == 1 => {
                    let d = &d.declarations[0];
                    let BindingPattern::BindingIdentifier(id) = &d.id else {
                        return None;
                    };
                    if id.symbol_id.get() != Some(counter) {
                        return None;
                    }
                    d.init.as_ref()?
                }
                Some(ForStatementInit::AssignmentExpression(a))
                    if a.operator == AssignmentOperator::Assign =>
                {
                    let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left else {
                        return None;
                    };
                    if self
                        .scopes
                        .get_reference(id.reference_id.get()?)
                        .symbol_id()
                        != Some(counter)
                    {
                        return None;
                    }
                    &a.right
                }
                None => self.definition(counter, Span::new(n.span.start, n.span.start))?,
                _ => return None,
            };
            let update = if let Some(update) = n.update {
                let Expression::UpdateExpression(update) = update.without_parentheses() else {
                    return None;
                };
                update.as_ref()
            } else {
                let updates: Vec<_> = self.index.updates.iter().copied().filter(|u| contains(n.body.span(),u.span) && {
                matches!(&u.argument, SimpleAssignmentTarget::AssignmentTargetIdentifier(id) if id.reference_id.get().and_then(|r| self.scopes.get_reference(r).symbol_id()) == Some(counter))
            }).collect();
                if updates.len() != 1 {
                    return None;
                }
                let update = updates[0];
                if self
                    .index
                    .guards
                    .iter()
                    .any(|g| contains(*g, update.span) && !contains(*g, loop_span))
                    || self
                        .index
                        .loops
                        .iter()
                        .any(|inner| inner.span != n.span && contains(inner.span, update.span))
                {
                    return None;
                }
                update
            };
            let SimpleAssignmentTarget::AssignmentTargetIdentifier(id) = &update.argument else {
                return None;
            };
            if self
                .scopes
                .get_reference(id.reference_id.get()?)
                .symbol_id()
                != Some(counter)
            {
                return None;
            }
            let (bound, direction) = if test.operator == BinaryOperator::LessThan
                && update.operator == UpdateOperator::Increment
                && matches!(start.without_parentheses(), Expression::NumericLiteral(v) if v.value == 0.)
            {
                (&test.right, LoopDirection::Ascending)
            } else if test.operator == BinaryOperator::GreaterEqualThan
                && update.operator == UpdateOperator::Decrement
                && matches!(test.right.without_parentheses(), Expression::NumericLiteral(v) if v.value == 0.)
            {
                let Expression::BinaryExpression(start) = start.without_parentheses() else {
                    return None;
                };
                if start.operator != BinaryOperator::Subtraction
                    || !matches!(start.right.without_parentheses(), Expression::NumericLiteral(v) if v.value == 1.)
                {
                    return None;
                }
                (&start.left, LoopDirection::Descending)
            } else {
                return None;
            };
            (counter, update, bound, direction, None)
        };
        *refusal = CountRefusal::ControlTransfer;
        if self
            .index
            .transfers
            .iter()
            .any(|s| contains(n.body.span(), *s) && Some(*s) != exit)
        {
            return None;
        }
        *refusal = CountRefusal::Mutation;
        let owner = |s| {
            self.index
                .functions
                .iter()
                .copied()
                .filter(|f| contains(*f, s))
                .min_by_key(|f| f.size())
        };
        let function = owner(n.span)?;
        if self.index.dynamic.iter().any(|s| contains(function, *s))
            || self.scopes.get_resolved_references(counter).any(|r| {
                r.is_write() && owner(self.nodes.get_node(r.node_id()).span()) != Some(function)
            })
        {
            return None;
        }
        if self.scopes.get_resolved_references(counter).any(|r| {
            r.is_write() && {
                let s = self.nodes.get_node(r.node_id()).span();
                contains(n.body.span(), s) && !contains(update.span, s)
            }
        }) {
            return None;
        }
        *refusal = CountRefusal::ReachingBound;
        let (collection, bound_evaluation) = self.length(bound, n.span, 0)?;
        *refusal = CountRefusal::PriorWireCount;
        let (path, write) = prior.iter().rev().find(|(_, w)| {
            self.index
                .expressions
                .iter()
                .find(|e| e.span() == w.value_span)
                .and_then(|e| self.length(e, w.span, 0))
                .is_some_and(|(s, _)| s == collection)
                && w.span.end < n.span.start
                && self.block(w.span) == self.block(n.span)
        })?;
        *refusal = CountRefusal::StableLength;
        let value = self
            .index
            .expressions
            .iter()
            .find(|e| e.span() == write.value_span)?;
        let (_, count_evaluation) = self.length(value, write.span, 0)?;
        let same_cached_evaluation = count_evaluation == bound_evaluation;
        let origin = self
            .index
            .expressions
            .iter()
            .find(|e| e.span() == count_evaluation)?;
        let Expression::StaticMemberExpression(origin) = origin.without_parentheses() else {
            return None;
        };
        let array = crate::signals::extract::reaching::expression_value_at_use(
            self.scopes,
            self.nodes,
            &origin.object,
        )
        .ok()
        .map(|d| d.value)
        .filter(|e| matches!(e.without_parentheses(), Expression::ArrayExpression(_)));
        // Equal reaching evaluations reuse the very same cached scalar. A
        // repeated property read additionally needs a stable, unescaped Array.
        if !same_cached_evaluation {
            let array = array?;
            let owner = |s| {
                self.index
                    .functions
                    .iter()
                    .copied()
                    .filter(|f| contains(*f, s))
                    .min_by_key(|f| f.size())
            };
            if self.scopes.get_resolved_references(collection).any(|r| {
                let s = self.nodes.get_node(r.node_id()).span();
                if owner(s) != owner(n.span) {
                    return true;
                }
                if s.start < array.span().end || s.start >= n.span.end {
                    return false;
                }
                if r.is_write() {
                    return true;
                }
                !self
                    .index
                    .expressions
                    .iter()
                    .any(|e| match e.without_parentheses() {
                        Expression::StaticMemberExpression(m) => {
                            m.object.span() == s && m.property.name == "length" && !m.optional
                        }
                        Expression::ComputedMemberExpression(m) => {
                            m.object.span() == s && !m.optional
                        }
                        _ => false,
                    })
            }) {
                return None;
            }
        }
        // Array element assignments are absent from expression member nodes;
        // the reference check above rejects them after allocation as well.
        let mut writes = Vec::new();
        emission.grammar.walk(&mut |g| {
            if let GrammarNode::Number(w) | GrammarNode::Byte(w) | GrammarNode::String(w) = g {
                writes.push(w);
            }
        });
        *refusal = CountRefusal::UnaccountedCall;
        if self.index.calls.iter().any(|call| {
            contains(n.body.span(), call.span)
                && !writes.iter().any(|w| {
                    (w.span == call.span && !call.optional)
                        || (contains(w.span, call.span)
                            && call.arguments.is_empty()
                            && !call.optional
                            && self.symbol(&call.callee).is_some_and(|symbol| {
                                self.index.definitions.iter().any(|(s, _, value)| {
                                    *s == symbol
                                        && w.stream_instance.is_some_and(|instance| {
                                            value.span().start == instance.start
                                                && value.span().end == instance.end
                                        })
                                })
                            }))
                })
        }) {
            return None;
        }
        Some(CountRef {
            path: path.clone(),
            repeat_path: Vec::new(),
            write: write.span.into(),
            collection_binding: self.scopes.symbol_span(collection).into(),
            counter_binding: self.scopes.symbol_span(counter).into(),
            update: update.span.into(),
            update_in_body: contains(n.body.span(), update.span),
            predecrement_exit: exit.map(Into::into),
            length_evaluation: count_evaluation.into(),
            same_cached_evaluation,
            loop_span: n.span.into(),
            direction,
            maximum: 65536,
            reject_negative_zero: array.is_some(),
        })
    }
}

pub(super) fn source_plan(
    source: &str,
    emission: &EmissionContract,
    algorithms: &super::AlgorithmReport,
) -> SourcePlan {
    let mut needs_proof = false;
    emission.grammar.walk(&mut |g| {
        if matches!(
            g,
            GrammarNode::Choice { .. } | GrammarNode::Repeat { count: None, .. }
        ) {
            needs_proof = true
        }
    });
    if !needs_proof {
        return SourcePlan::default();
    }
    let arena = Allocator::default();
    let parsed = Parser::new(&arena, source, SourceType::mjs()).parse();
    if !parsed.errors.is_empty() {
        return SourcePlan::default();
    }
    let semantic = oxc_semantic::SemanticBuilder::new()
        .build(&parsed.program)
        .semantic;
    let mut index = Index::default();
    index.visit_program(&parsed.program);
    // Assignment reaching definitions use the same resolved symbol table.
    for e in &index.expressions {
        if let Expression::AssignmentExpression(a) = e.without_parentheses() {
            if a.operator == AssignmentOperator::Assign {
                if let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left {
                    if let Some(s) = id
                        .reference_id
                        .get()
                        .and_then(|r| semantic.scoping().get_reference(r).symbol_id())
                    {
                        index.definitions.push((s, a.span, &a.right));
                    }
                }
            }
        }
    }
    let proof = Proof {
        index,
        scopes: semantic.scoping(),
        nodes: semantic.nodes(),
    };
    fn scan<'a>(
        g: &'a GrammarNode,
        path: &mut Vec<usize>,
        prior: &mut Vec<(
            Vec<usize>,
            &'a crate::signals::extract::contracts::WriteNode,
        )>,
        p: &Proof<'_, '_>,
        e: &EmissionContract,
        out: &mut SourcePlan,
    ) {
        match g {
            GrammarNode::Sequence { items } => {
                for (i, g) in items.iter().enumerate() {
                    path.push(i);
                    scan(g, path, prior, p, e, out);
                    path.pop();
                }
            }
            GrammarNode::Number(w) => prior.push((path.clone(), w)),
            GrammarNode::Repeat {
                span,
                count: None,
                direction,
                ..
            } => {
                let mut reason = CountRefusal::Induction;
                if let Some(mut c) = p.prove(*span, prior, e, &mut reason) {
                    c.repeat_path = path.iter().copied().map(ControlStep::Child).collect();
                    if *direction == LoopDirection::Unknown || c.direction == *direction {
                        out.counts.push(c);
                    } else {
                        out.unproven_counts.push(UnprovenCount {
                            path: path.iter().copied().map(ControlStep::Child).collect(),
                            loop_span: (*span).into(),
                            reason: CountRefusal::Induction,
                        });
                    }
                } else {
                    out.unproven_counts.push(UnprovenCount {
                        path: path.iter().copied().map(ControlStep::Child).collect(),
                        loop_span: (*span).into(),
                        reason,
                    });
                }
            }
            // Branch-local and repeated scalar identities are deliberately not
            // exported into the unconditional prior-value environment.
            _ => {}
        }
    }
    let mut out = SourcePlan {
        root_cases: root_emissions(source, emission)
            .map(|v| v.into_iter().map(|(p, _)| p).collect())
            .unwrap_or_default(),
        ..SourcePlan::default()
    };
    scan(
        &emission.grammar,
        &mut vec![],
        &mut vec![],
        &proof,
        emission,
        &mut out,
    );
    emission.grammar.walk(&mut |g| {
        use crate::signals::extract::contracts::ChoiceKind;
        let GrammarNode::Choice {
            kind: ChoiceKind::If,
            span,
            arms,
            ..
        } = g
        else {
            return;
        };
        if arms.len() != 2 || arms[0].label != "then" || arms[1].label != "else" {
            return;
        }
        let path = grammar_path(&emission.grammar, g).unwrap();
        let Some(count) = out.counts.iter().find(|c| {
            let mut body = c.repeat_path.clone();
            body.push(ControlStep::Body);
            contains(Span::new(c.loop_span.start, c.loop_span.end), *span)
                && path.starts_with(&body)
        }) else {
            return;
        };
        let Some(test) = semantic.nodes().iter().find_map(|n| match n.kind() {
            oxc_ast::AstKind::IfStatement(n) if n.span == *span => Some(&n.test),
            _ => None,
        }) else {
            return;
        };
        if count.update_in_body
            && count.predecrement_exit.is_none()
            && test.span().start >= count.update.start
        {
            return;
        }
        let Expression::BinaryExpression(compare) = test.without_parentheses() else {
            return;
        };
        let inverted = match compare.operator {
            BinaryOperator::StrictEquality => false,
            BinaryOperator::StrictInequality => true,
            _ => return,
        };
        let Expression::BinaryExpression(bits) = compare.left.without_parentheses() else {
            return;
        };
        if bits.operator != BinaryOperator::BitwiseAnd {
            return;
        }
        let Some(counter) = proof.symbol(&bits.left) else {
            return;
        };
        if SourceSpan::from(semantic.scoping().symbol_span(counter)) != count.counter_binding {
            return;
        }
        let integer = |e: &Expression<'_>| match e.without_parentheses() {
            Expression::NumericLiteral(n)
                if n.value >= 0. && n.value <= 65535. && n.value.fract() == 0. =>
            {
                Some(n.value as u32)
            }
            _ => None,
        };
        let Some(mask) = integer(&bits.right) else {
            return;
        };
        let Some(equals) = integer(&compare.right) else {
            return;
        };
        out.counter_choices.push(CounterChoiceProof {
            choice: (*span).into(),
            read: test.span().into(),
            path: path.clone(),
            counter: CounterRef {
                loop_span: count.loop_span,
                repeat_path: count.repeat_path.clone(),
                binding: count.counter_binding,
            },
            predicate: BitPredicate {
                mask,
                equals,
                inverted,
            },
            input: CounterInput::Induction,
        });
    });
    for count in &out.counts {
        out.counter_choices
            .extend(proof.packed_choices(count, emission));
    }
    let mut choices = Vec::new();
    emission.grammar.walk(&mut |g| {
        let GrammarNode::Choice { span, arms, .. } = g else {
            return;
        };
        let Ok(super::Op::DomainChoice {
            domains,
            mask,
            consumption,
            ..
        }) = super::program::compile_node(g, emission, algorithms, &out, source, 0)
        else {
            return;
        };
        let semantics = (|| {
            let test = semantic.nodes().iter().find_map(|n| match n.kind() {
                oxc_ast::AstKind::IfStatement(n) if n.span == *span => Some(&n.test),
                oxc_ast::AstKind::ConditionalExpression(n) if n.span == *span => Some(&n.test),
                _ => None,
            })?;
            let Expression::BinaryExpression(test) = test.without_parentheses() else {
                return None;
            };
            let (nullish, literal_arm) = match test.operator {
                BinaryOperator::Equality => (true, 0),
                BinaryOperator::Inequality => (true, 1),
                BinaryOperator::StrictEquality => (false, 0),
                BinaryOperator::StrictInequality => (false, 1),
                _ => return None,
            };
            if !matches!(test.right.without_parentheses(), Expression::NullLiteral(_))
                || arms.len() != 2
            {
                return None;
            }
            let symbol = proof.symbol(&test.left)?;
            let GrammarNode::Byte(_) = &arms[literal_arm].body else {
                return None;
            };
            let w = match &arms[1 - literal_arm].body {
                GrammarNode::Number(w) | GrammarNode::String(w) => w,
                _ => return None,
            };
            let value = proof
                .index
                .expressions
                .iter()
                .find(|e| e.span() == w.value_span)?;
            if proof.symbol(value) != Some(symbol) {
                return None;
            }
            if !proof.stable_reads(symbol, test.left.span(), value.span()) {
                return None;
            }
            let binding = semantic.scoping().symbol_span(symbol).into();
            Some(if nullish {
                ChoiceSemantics::Nullish {
                    binding,
                    literal_arm,
                    guard_read: test.left.span().into(),
                    value_read: value.span().into(),
                }
            } else {
                ChoiceSemantics::NullOnly {
                    binding,
                    literal_arm,
                    guard_read: test.left.span().into(),
                    value_read: value.span().into(),
                }
            })
        })();
        choices.push(ChoiceDomainProof {
            choice: (*span).into(),
            path: grammar_path(&emission.grammar, g).unwrap(),
            domains,
            mask,
            consumption,
            semantics,
        });
    });
    out.choices = choices;
    out
}

fn following(repeat: &[ControlStep], choice: &[ControlStep]) -> bool {
    if repeat.len() != choice.len() || repeat.is_empty() {
        return false;
    }
    let n = repeat.len() - 1;
    repeat[..n] == choice[..n]
        && matches!((&repeat[n],&choice[n]),(ControlStep::Child(a),ControlStep::Child(b)) if b>a)
}

/// Source branch context for a root selected by its certified wire language.
/// Only the exact sibling arm is excluded; post-join uses remain obligations.
#[derive(Clone)]
pub(super) struct RootScope {
    pub choice: Span,
    pub region: Span,
    pub siblings: Vec<Span>,
}
impl RootScope {
    pub fn includes(&self, span: Span) -> bool {
        !self.siblings.iter().any(|region| contains(*region, span))
    }
}
pub(super) fn root_scope(
    source: &str,
    plan: &RootCasePlan,
    emission: SourceSpan,
) -> Option<RootScope> {
    let choice = plan.source_choice?;
    let arena = Allocator::default();
    let parsed = Parser::new(&arena, source, SourceType::mjs()).parse();
    if !parsed.errors.is_empty() {
        return None;
    }
    let semantic = oxc_semantic::SemanticBuilder::new()
        .build(&parsed.program)
        .semantic;
    let regions = semantic.nodes().iter().find_map(|node| {
        if node.span() != Span::new(choice.start, choice.end) {
            return None;
        }
        match node.kind() {
            oxc_ast::AstKind::IfStatement(statement) => Some(vec![
                statement.consequent.span(),
                statement.alternate.as_ref()?.span(),
            ]),
            oxc_ast::AstKind::ConditionalExpression(expression) => Some(vec![
                expression.consequent.span(),
                expression.alternate.span(),
            ]),
            _ => None,
        }
    })?;
    let region = *regions.get(plan.arm)?;
    if !contains(region, Span::new(plan.prefix.start, plan.prefix.end))
        || !contains(region, Span::new(plan.stream.start, plan.stream.end))
    {
        return None;
    }
    let site = Span::new(emission.start, emission.end);
    let choice = Span::new(choice.start, choice.end);
    if contains(choice, site) && !contains(region, site) {
        return None;
    }
    Some(RootScope {
        choice,
        region,
        siblings: regions
            .into_iter()
            .filter(|other| *other != region)
            .collect(),
    })
}
