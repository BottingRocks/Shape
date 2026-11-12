//! Source proofs for a concrete stream invocation in one supplied initialization
//! epoch. These prerequisites are separate from whole-body algorithm identity.
use super::*;
use crate::signals::{
    extract::contracts::{EmissionContract, GrammarNode},
    schema::SourceIdentity,
};
use oxc_semantic::{AstNodes, NodeId, SymbolId};
use oxc_syntax::operator::{AssignmentOperator, BinaryOperator};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StreamReadiness {
    pub source: SourceIdentity,
    pub body: AlgorithmProof,
    pub state_binding: SourceSpan,
    pub hash_assignment: SourceSpan,
    pub initializer: SourceSpan,
    pub public_entry: SourceSpan,
    pub constructor_call: SourceSpan,
    pub initializer_call: SourceSpan,
    pub public_api_return: SourceSpan,
    pub initializer_sha256: String,
    pub instance: SourceSpan,
    pub prefix: SourceSpan,
    pub seed_binding: SourceSpan,
    pub emission: SourceSpan,
    pub activation: Vec<SourceSpan>,
    /// The caller must associate the report with this supplied initializer,
    /// before another initialization of this source's shared state. This is a
    /// protocol requirement, not an invented value for an unknown PRNG state.
    pub required_context: InitializationEpoch,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InitializationEpoch {
    SuppliedPublicInitializerWithoutInterveningInitialization,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StreamReadinessError(pub &'static str);
impl std::fmt::Display for StreamReadinessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}
impl std::error::Error for StreamReadinessError {}
type Result<T> = std::result::Result<T, StreamReadinessError>;
fn fail<T>(why: &'static str) -> Result<T> {
    Err(StreamReadinessError(why))
}

/// A statically parsed call from the trusted initialization artifact. There is
/// no boolean/default constructor. Its token must equal the caller's pinned
/// session token; neither the token nor argument literals are retained.
pub struct InitializationContext {
    sha256: String,
    method: String,
}
impl InitializationContext {
    /// Digest of the trusted artifact; no token or literal argument is exposed.
    pub fn initializer_sha256(&self) -> &str {
        &self.sha256
    }

    pub fn from_initializer(script: &str, expected_token: &str) -> Result<Self> {
        let arena = Allocator::default();
        let parsed = Parser::new(&arena, script, SourceType::mjs()).parse();
        if !parsed.errors.is_empty() {
            return fail("initializer parse diagnostics");
        }
        let built = SemanticBuilder::new()
            .with_check_syntax_error(true)
            .build(&parsed.program);
        if !built.errors.is_empty() {
            return fail("initializer semantic diagnostics");
        }
        let calls:Vec<_>=built.semantic.nodes().iter().filter_map(|n| match n.kind() {
            AstKind::CallExpression(c) if matches!(c.callee.without_parentheses(),Expression::StaticMemberExpression(m)
                if matches!(m.object.without_parentheses(),Expression::StaticMemberExpression(d) if d.property.name=="detail"))=>Some(c),_=>None }).collect();
        let [call] = calls.as_slice() else {
            return fail("initializer must select one detail callback");
        };
        let Expression::StaticMemberExpression(member) = call.callee.without_parentheses() else {
            return fail("initializer callback is dynamic");
        };
        if call.optional || member.optional {
            return fail("initializer callback is conditional");
        }
        if !matches!(call.arguments.first().and_then(|a| a.as_expression()),Some(Expression::StringLiteral(s)) if s.value==expected_token)
        {
            return fail("initializer token differs from pinned session");
        }
        if call.arguments.iter().any(|a| a.as_expression().is_none()) {
            return fail("initializer spreads arguments");
        }
        Ok(Self {
            sha256: digest(script.as_bytes()),
            method: member.property.name.to_string(),
        })
    }
}

struct Graph<'a, 's> {
    scopes: &'s Scoping,
    nodes: &'s AstNodes<'a>,
}
impl<'a> Graph<'a, '_> {
    fn symbol(&self, id: &IdentifierReference<'_>) -> Option<SymbolId> {
        id.reference_id
            .get()
            .and_then(|r| self.scopes.get_reference(r).symbol_id())
    }
    fn expr_symbol(&self, e: &Expression<'_>) -> Option<SymbolId> {
        match e.without_parentheses() {
            Expression::Identifier(id) => self.symbol(id),
            _ => None,
        }
    }
    fn node(&self, span: SourceSpan) -> Option<NodeId> {
        self.nodes
            .iter()
            .find(|n| {
                !matches!(n.kind(), AstKind::Program(_)) && SourceSpan::from(n.span()) == span
            })
            .map(|n| n.id())
    }
    fn owner(&self, id: NodeId) -> Option<NodeId> {
        enclosing_activation(self.nodes, id)
    }
    fn function(&self, span: SourceSpan) -> Option<&'a Function<'a>> {
        self.nodes.iter().find_map(|n| match n.kind() {
            AstKind::Function(f) if SourceSpan::from(f.span) == span => Some(f),
            _ => None,
        })
    }
    fn formals(&self, id: NodeId) -> Vec<Option<SymbolId>> {
        crate::signals::extract::bindings::function_formal_symbols(self.nodes, id)
    }
    fn origin(&self, e: &Expression<'a>) -> Option<SourceSpan> {
        self.origin_depth(e, 0)
    }
    fn origin_depth(&self, e: &Expression<'a>, depth: usize) -> Option<SourceSpan> {
        if depth > 16 {
            return None;
        }
        match e.without_parentheses() {
            Expression::FunctionExpression(f) => Some(f.span.into()),
            Expression::Identifier(id) => {
                let symbol = self.symbol(id)?;
                if !self.scopes.symbol_redeclarations(symbol).is_empty() {
                    return None;
                }
                if let AstKind::Function(f) =
                    self.nodes.kind(self.scopes.symbol_declaration(symbol))
                {
                    return (!self
                        .scopes
                        .get_resolved_references(symbol)
                        .any(|r| r.is_write()))
                    .then_some(f.span.into());
                }
                self.origin_depth(self.value(symbol)?, depth + 1)
            }
            _ => None,
        }
    }
    fn value(&self, symbol: SymbolId) -> Option<&'a Expression<'a>> {
        if !self.scopes.symbol_redeclarations(symbol).is_empty() {
            return None;
        }
        let AstKind::VariableDeclarator(d) =
            self.nodes.kind(self.scopes.symbol_declaration(symbol))
        else {
            return None;
        };
        let writes: Vec<_> = self
            .scopes
            .get_resolved_references(symbol)
            .filter(|r| r.is_write())
            .collect();
        if let Some(e) = &d.init {
            return writes.is_empty().then_some(e);
        }
        let [write] = writes.as_slice() else {
            return None;
        };
        let AstKind::AssignmentExpression(a) =
            self.nodes.kind(self.nodes.parent_id(write.node_id()))
        else {
            return None;
        };
        (a.operator == AssignmentOperator::Assign
            && matches!(
                self.nodes
                    .kind(self.nodes.parent_id(self.nodes.parent_id(write.node_id()))),
                AstKind::ExpressionStatement(_)
            ))
        .then_some(&a.right)
    }
    fn object_root(&self, e: &Expression<'a>, depth: usize) -> Option<SymbolId> {
        if depth > 16 {
            return None;
        }
        let s = self.expr_symbol(e)?;
        match self.value(s)?.without_parentheses() {
            Expression::Identifier(_) => self.object_root(self.value(s)?, depth + 1),
            Expression::ObjectExpression(_) => Some(s),
            _ => None,
        }
    }
    fn call(&self, span: SourceSpan) -> Option<&'a CallExpression<'a>> {
        self.nodes.iter().find_map(|n| match n.kind() {
            AstKind::CallExpression(c) if SourceSpan::from(c.span) == span => Some(c),
            _ => None,
        })
    }
    fn whole_statement(&self, id: NodeId) -> bool {
        let p = self.nodes.parent_id(id);
        matches!(self.nodes.kind(p), AstKind::ExpressionStatement(_))
            && matches!(
                self.nodes.kind(self.nodes.parent_id(p)),
                AstKind::FunctionBody(_)
            )
    }
    fn dominates(&self, definition: NodeId, usage: NodeId) -> bool {
        let mut statement = definition;
        while !matches!(
            self.nodes.kind(statement),
            AstKind::ExpressionStatement(_) | AstKind::VariableDeclaration(_)
        ) {
            let p = self.nodes.parent_id(statement);
            if p == statement {
                return false;
            }
            statement = p;
            if matches!(
                self.nodes.kind(statement),
                AstKind::Function(_) | AstKind::Program(_)
            ) {
                return false;
            }
        }
        let list = self.nodes.parent_id(statement);
        if !matches!(
            self.nodes.kind(list),
            AstKind::FunctionBody(_) | AstKind::BlockStatement(_)
        ) {
            return false;
        }
        let mut cursor = usage;
        loop {
            let p = self.nodes.parent_id(cursor);
            if p == list {
                return self.nodes.get_node(statement).span().end
                    <= self.nodes.get_node(cursor).span().start;
            }
            if p == cursor {
                return false;
            }
            cursor = p;
        }
    }
    /// Every exposure of this function activation must follow initialization.
    /// Inline callbacks are exposed where their containing call evaluates them;
    /// declarations/aliases are values until invoked or handed to another call.
    fn after(
        &self,
        id: NodeId,
        definition: &StateDefinition,
        stack: &mut Vec<NodeId>,
        path: &mut Vec<SourceSpan>,
    ) -> bool {
        let Some(owner) = self.owner(id) else {
            return false;
        };
        if self.nodes.get_node(owner).span()
            == oxc_span::Span::new(definition.initializer.start, definition.initializer.end)
        {
            return self.nodes.get_node(id).span().start >= definition.assignment.end;
        }
        if stack.contains(&owner) {
            return false;
        }
        stack.push(owner);
        let binding = match self.nodes.kind(owner) {
            AstKind::Function(f) if f.r#type == FunctionType::FunctionDeclaration => {
                f.id.as_ref().and_then(|i| i.symbol_id.get())
            }
            _ => None,
        };
        let mut pending = Vec::new();
        if let Some(s) = binding {
            pending.push(s)
        } else {
            let parent = self.nodes.parent_id(owner);
            if let AstKind::VariableDeclarator(d) = self.nodes.kind(parent) {
                if let BindingPattern::BindingIdentifier(b) = &d.id {
                    if let Some(s) = b.symbol_id.get() {
                        pending.push(s)
                    }
                }
            }
        }
        let ok = if pending.is_empty() {
            // Function/arrow used directly as an argument; its body can run
            // no earlier than exposure. No claim about when it actually runs.
            let parent = self.nodes.parent_id(owner);
            matches!(
                self.nodes.kind(parent),
                AstKind::CallExpression(c) if matches!(c.callee.without_parentheses(),Expression::StaticMemberExpression(m)
                    if m.property.name=="forEach" && self.expr_symbol(&m.object).and_then(|s|self.value(s)).is_some_and(|e|matches!(e.without_parentheses(),Expression::ArrayExpression(_))))
            ) && self.after(owner, definition, stack, path)
        } else {
            let mut seen = Vec::new();
            let mut exposed = false;
            let mut valid = true;
            while let Some(s) = pending.pop() {
                if seen.contains(&s) {
                    valid = false;
                    break;
                }
                seen.push(s);
                for r in self.scopes.get_resolved_references(s) {
                    if r.is_write() {
                        valid = false;
                        break;
                    }
                    let node = r.node_id();
                    let span = self.nodes.get_node(node).span();
                    match self.nodes.kind(self.nodes.parent_id(node)) {
                        AstKind::VariableDeclarator(d)
                            if d.init.as_ref().is_some_and(|e| e.span() == span) =>
                        {
                            if let BindingPattern::BindingIdentifier(b) = &d.id {
                                if let Some(s) = b.symbol_id.get() {
                                    pending.push(s);
                                    continue;
                                }
                            }
                            valid = false;
                        }
                        AstKind::CallExpression(c) if c.callee.span() == span => {
                            exposed = true;
                            if !self.after(node, definition, stack, path) {
                                valid = false
                            }
                        }
                        _ => valid = false,
                    }
                }
            }
            valid && exposed
        };
        stack.pop();
        if ok {
            path.push(self.nodes.get_node(owner).span().into())
        }
        ok
    }
}

impl AlgorithmReport {
    /// Regenerate a deserialized witness before trusting its source-bound fields.
    pub fn verify_stream_readiness(
        &self,
        proof: &StreamReadiness,
        source: &str,
        encoding: &EncodingProperties,
        wiring: &WiringProperties,
        emission: &EmissionContract,
        context: &InitializationContext,
    ) -> Result<()> {
        let regenerated =
            self.certify_stream_instance(source, encoding, wiring, emission, context)?;
        if regenerated != *proof {
            return fail("serialized stream readiness differs");
        };
        Ok(())
    }
    /// Certify one concrete invocation and its source-backed initialization
    /// prerequisite. A legacy global refusal is not silently changed to true.
    /// Consumers must regenerate and compare this proof before execution.
    pub fn certify_stream_instance(
        &self,
        source: &str,
        encoding: &EncodingProperties,
        wiring: &WiringProperties,
        emission: &EmissionContract,
        context: &InitializationContext,
    ) -> Result<StreamReadiness> {
        let identity = SourceIdentity {
            sha256: digest(source.as_bytes()),
            bytes: source.len(),
        };
        if encoding.source != identity || wiring.source != identity {
            return fail("source identity differs");
        }
        if *self != certify(source, encoding, wiring) || self.hash.codec.is_none() {
            return fail("algorithm dependencies differ");
        }
        let arena = Allocator::default();
        let parsed = Parser::new(&arena, source, SourceType::mjs()).parse();
        if !parsed.errors.is_empty() {
            return fail("source parse diagnostics");
        }
        let built = SemanticBuilder::new()
            .with_check_syntax_error(true)
            .build(&parsed.program);
        if !built.errors.is_empty() {
            return fail("source semantic diagnostics");
        }
        let semantic = built.semantic;
        let g = Graph {
            scopes: semantic.scoping(),
            nodes: semantic.nodes(),
        };
        let stream = emission
            .stream
            .as_ref()
            .ok_or(StreamReadinessError("emission stream missing"))?;
        let instance = stream
            .instance
            .ok_or(StreamReadinessError("concrete stream instance missing"))?;
        let call = g
            .call(instance)
            .ok_or(StreamReadinessError("instance is not a call"))?;
        let factory = g
            .origin(&call.callee)
            .ok_or(StreamReadinessError("instance factory origin is ambiguous"))?;
        let body = proof(
            source
                .get(factory.start as usize..factory.end as usize)
                .map(|s| (factory, s)),
            include_str!("reference/stream.js"),
            "sum-rot26-9-13-le-burn4/1",
            false,
        );
        if body.codec.is_none() {
            return fail("stream body differs");
        }
        let token = wiring
            .public_init
            .as_ref()
            .and_then(|p| p.token.as_ref())
            .ok_or(StreamReadinessError("public token provenance missing"))?;
        let definition = stream_state_definition(
            source,
            factory,
            self.hash.span.ok_or(StreamReadinessError("hash missing"))?,
            token,
            wiring,
        )
        .map_err(StreamReadinessError)?;
        let mut prefixes = Vec::new();
        emission.grammar.walk(&mut |n| {
            if let GrammarNode::RawPrefix(p) = n {
                prefixes.push(p.clone())
            }
        });
        let [prefix] = prefixes.as_slice() else {
            return fail("instance needs one concrete prefix");
        };
        let seed = stream
            .seed_binding
            .ok_or(StreamReadinessError("stream seed missing"))?;
        if prefix.seed_binding != Some(seed) {
            return fail("prefix and stream seed differ");
        }
        verify_prefix(&g, prefix.span.into(), seed, call, emission.site)?;
        let instance_node = g
            .node(instance)
            .ok_or(StreamReadinessError("instance node missing"))?;
        let emission_node = g
            .node(emission.site)
            .ok_or(StreamReadinessError("emission node missing"))?;
        if g.owner(instance_node) != g.owner(emission_node) || instance.end >= emission.site.start {
            return fail("instance is not local to emission");
        }
        let (public_entry, constructor_call, initializer_call, public_api_return) =
            verify_public_entry(&g, wiring, &definition, context)?;
        let mut activation = Vec::new();
        verify_activation(
            &g,
            wiring,
            instance_node,
            emission_node,
            &definition,
            &mut activation,
            &mut Vec::new(),
        )?;
        activation.sort_by_key(|s| (s.start, s.end));
        activation.dedup();
        Ok(StreamReadiness {
            source: identity,
            body,
            state_binding: definition.state,
            hash_assignment: definition.assignment,
            initializer: definition.initializer,
            public_entry,
            constructor_call,
            initializer_call,
            public_api_return,
            initializer_sha256: context.sha256.clone(),
            instance,
            prefix: prefix.span.into(),
            seed_binding: seed,
            emission: emission.site,
            activation,
            required_context:
                InitializationEpoch::SuppliedPublicInitializerWithoutInterveningInitialization,
        })
    }
}

fn verify_prefix(
    g: &Graph<'_, '_>,
    span: SourceSpan,
    seed: SourceSpan,
    call: &CallExpression<'_>,
    emission: SourceSpan,
) -> Result<()> {
    let Some(node) = g.node(span) else {
        return fail("prefix source missing");
    };
    let AstKind::ArrayExpression(array) = g.nodes.kind(node) else {
        return fail("prefix is not a source array");
    };
    let write = g.nodes.parent_id(node);
    let buffer = match g.nodes.kind(write) {
        AstKind::VariableDeclarator(d) => match &d.id {
            BindingPattern::BindingIdentifier(b) => b.symbol_id.get(),
            _ => None,
        },
        AstKind::AssignmentExpression(a) if a.operator == AssignmentOperator::Assign => {
            match &a.left {
                AssignmentTarget::AssignmentTargetIdentifier(id) => g.symbol(id),
                _ => None,
            }
        }
        _ => None,
    }
    .ok_or(StreamReadinessError("prefix buffer binding missing"))?;
    let emit = g
        .call(emission)
        .ok_or(StreamReadinessError("emission is not a source call"))?;
    let Some(Expression::ArrayExpression(payload)) = emit
        .arguments
        .first()
        .and_then(|a| a.as_expression())
        .map(Expression::without_parentheses)
    else {
        return fail("emission payload is not source-bound");
    };
    if payload.elements.len() != 2
        || payload.elements[1]
            .as_expression()
            .and_then(|e| g.expr_symbol(e))
            != Some(buffer)
    {
        return fail("prefix belongs to another emitted buffer");
    }
    if !(array.span.end < call.span.start || call.span.end < array.span.start) {
        return fail("prefix and stream expressions overlap");
    }
    if array.elements.len() != 4 || call.arguments.len() != 1 {
        return fail("prefix or stream arity differs");
    }
    for (i, e) in array.elements.iter().enumerate() {
        let Some(mut e) = e.as_expression().map(Expression::without_parentheses) else {
            return fail("prefix contains a spread");
        };
        let mut mask = false;
        if let Expression::BinaryExpression(b) = e {
            if b.operator == BinaryOperator::BitwiseAnd
                && matches!(b.right.without_parentheses(),Expression::NumericLiteral(n) if n.value==255.)
            {
                mask = true;
                e = b.left.without_parentheses();
            }
        }
        if i > 0 && !mask {
            return fail("prefix byte narrowing differs");
        }
        if i < 3 {
            let Expression::BinaryExpression(b) = e else {
                return fail("prefix shift missing");
            };
            if !matches!(
                b.operator,
                BinaryOperator::ShiftRight | BinaryOperator::ShiftRightZeroFill
            ) || (!mask && b.operator != BinaryOperator::ShiftRightZeroFill)
                || !matches!(b.right.without_parentheses(),Expression::NumericLiteral(n) if n.value==((3-i)*8) as f64)
            {
                return fail("prefix byte order differs");
            };
            e = b.left.without_parentheses();
        }
        let s = g
            .expr_symbol(e)
            .ok_or(StreamReadinessError("prefix seed is not a binding"))?;
        if SourceSpan::from(g.scopes.symbol_span(s)) != seed {
            return fail("prefix seed binding differs");
        }
    }
    let seed_symbol = g
        .scopes
        .symbol_ids()
        .find(|s| SourceSpan::from(g.scopes.symbol_span(*s)) == seed)
        .ok_or(StreamReadinessError("seed binding missing"))?;
    if g.scopes.get_resolved_references(seed_symbol).any(|r| {
        r.is_write() && {
            let span = g.nodes.get_node(r.node_id()).span();
            let own_write=matches!(g.nodes.kind(g.nodes.parent_id(r.node_id())),AstKind::AssignmentExpression(a)
                if a.operator==AssignmentOperator::Assign && a.right.span()==call.span);
            // RHS evaluation precedes its assignment. Reusing the seed holder
            // is valid only after the prefix already captured the numeric seed.
            (own_write && call.span.end < array.span.start)
                || (array.span.start.min(call.span.start) < span.start
                    && span.end < array.span.end.max(call.span.end)
                    && !own_write)
        }
    }) {
        return fail("seed changes between prefix and stream");
    }
    let arg = call.arguments[0]
        .as_expression()
        .ok_or(StreamReadinessError("stream argument is spread"))?;
    let Expression::BinaryExpression(b) = arg.without_parentheses() else {
        return fail("stream seed/salt composition missing");
    };
    if b.operator != BinaryOperator::BitwiseXOR
        || !matches!(b.right.without_parentheses(), Expression::NumericLiteral(_))
        || g.expr_symbol(&b.left)
            .is_none_or(|s| SourceSpan::from(g.scopes.symbol_span(s)) != seed)
    {
        return fail("stream seed/salt binding differs");
    }
    Ok(())
}

/// Prove the value consumed by the hash, independently of descriptive wiring.
/// Both global certification and regenerated instance witnesses pass this gate.
/// Unknown object consumers are escapes: a matching argument spelling and an
/// exact byte reencode cannot establish the native stream's token value.
pub(super) fn verify_token_value<'a>(
    input: &Expression<'a>,
    hash: SourceSpan,
    definition: &StateDefinition,
    scopes: &Scoping,
    nodes: &AstNodes<'a>,
    wiring: &WiringProperties,
) -> Result<()> {
    let g = Graph { scopes, nodes };
    let (object, property) = match input.without_parentheses() {
        Expression::StaticMemberExpression(m) if !m.optional => {
            (&m.object, m.property.name.as_str())
        }
        Expression::ComputedMemberExpression(m) if !m.optional => {
            let Expression::StringLiteral(key) = m.expression.without_parentheses() else {
                return fail("hash token key is dynamic");
            };
            (&m.object, key.value.as_str())
        }
        _ => return fail("hash token input is not a local property"),
    };
    let root = g.object_root(object, 0).ok_or(StreamReadinessError(
        "hash token object origin is ambiguous",
    ))?;
    let declaration = scopes.symbol_declaration(root);
    let constructor = g
        .owner(declaration)
        .ok_or(StreamReadinessError("hash token object has no constructor"))?;
    let arg0 = g
        .formals(constructor)
        .first()
        .copied()
        .flatten()
        .ok_or(StreamReadinessError("token constructor formal is missing"))?;
    let read = g
        .node(hash)
        .ok_or(StreamReadinessError("hash read is missing"))?;
    let read_owner = g
        .owner(read)
        .ok_or(StreamReadinessError("hash activation is missing"))?;

    // A callback declaration can precede its initialization text. Its references,
    // rather than its declaration position, bound its earliest possible exposure.
    let anchors = if read_owner == constructor {
        vec![read]
    } else {
        if g.owner(read_owner) != Some(constructor) {
            return fail("hash callback is not local to the token constructor");
        }
        let binding = match nodes.kind(read_owner) {
            AstKind::Function(f) if f.r#type == FunctionType::FunctionDeclaration => {
                f.id.as_ref().and_then(|id| id.symbol_id.get())
            }
            _ => None,
        };
        if let Some(binding) = binding {
            let refs: Vec<_> = scopes.get_resolved_references(binding).collect();
            if refs.is_empty()
                || refs
                    .iter()
                    .any(|r| r.is_write() || g.owner(r.node_id()) != Some(constructor))
            {
                return fail("hash callback has unresolved exposure");
            }
            refs.iter().map(|r| r.node_id()).collect()
        } else {
            vec![read_owner]
        }
    };
    let before_read = |definition| anchors.iter().all(|use_| g.dominates(definition, *use_));
    // A later statement in this activation is irrelevant. For another closure,
    // require the existing activation proof for every invocation/exposure;
    // textual order of its declaration alone is insufficient.
    let after_read = |use_| {
        if g.owner(use_) == Some(read_owner) {
            g.dominates(read, use_)
        } else {
            g.after(use_, definition, &mut vec![], &mut vec![])
        }
    };
    let Some(Expression::ObjectExpression(literal)) =
        g.value(root).map(Expression::without_parentheses)
    else {
        return fail("hash token object is not a plain local object");
    };
    if !before_read(declaration) {
        return fail("hash token object does not dominate its read");
    }
    for p in &literal.properties {
        let ObjectPropertyKind::ObjectProperty(p) = p else {
            return fail("hash token object contains a spread");
        };
        if p.computed
            || p.kind != PropertyKind::Init
            || p.method
            || p.key.static_name().as_deref() == Some("__proto__")
        {
            return fail("hash token object has unresolved property effects");
        }
    }

    let aliases: Vec<_> = scopes
        .symbol_ids()
        .filter(|s| {
            *s == root
                || g.value(*s)
                    .is_some_and(|e| g.object_root(e, 0) == Some(root))
        })
        .collect();
    let mut stores = Vec::new();
    // Object literals evaluate properties in order; only the final own token
    // property can supply a reaching value. A placeholder followed by the
    // constructor's explicit store is handled by the same reference walk.
    if literal
        .properties
        .iter()
        .filter_map(|p| match p {
            ObjectPropertyKind::ObjectProperty(p)
                if p.key.static_name().as_deref() == Some(property) =>
            {
                Some(p)
            }
            _ => None,
        })
        .next_back()
        .is_some_and(|p| g.expr_symbol(&p.value) == Some(arg0))
    {
        stores.push(declaration);
    }
    // Every reference to the object (and each immutable alias) must be accounted
    // for. This includes passing it to helpers, mutators, methods, returns,
    // destructuring and mutable aliases, not just syntactic member assignments.
    for symbol in &aliases {
        for r in scopes.get_resolved_references(*symbol) {
            let mut use_ = r.node_id();
            if after_read(use_) {
                continue;
            }
            if r.is_write() {
                return fail("hash token object binding is overwritten");
            }
            while matches!(
                nodes.kind(nodes.parent_id(use_)),
                AstKind::ParenthesizedExpression(_)
            ) {
                use_ = nodes.parent_id(use_);
            }
            let parent = nodes.parent_id(use_);
            if let AstKind::VariableDeclarator(d) = nodes.kind(parent) {
                if let BindingPattern::BindingIdentifier(b) = &d.id {
                    if b.symbol_id.get().is_some_and(|s| aliases.contains(&s))
                        && before_read(parent)
                    {
                        continue;
                    }
                }
            }
            let (key, member_object) = match nodes.kind(parent) {
                AstKind::StaticMemberExpression(m) => (Some(m.property.name.as_str()), &m.object),
                AstKind::ComputedMemberExpression(m) => (
                    match m.expression.without_parentheses() {
                        Expression::StringLiteral(s) => Some(s.value.as_str()),
                        _ => None,
                    },
                    &m.object,
                ),
                _ => return fail("hash token object escapes to an unresolved consumer"),
            };
            if g.expr_symbol(member_object) != Some(*symbol) {
                return fail("hash token object used as a computed key");
            }
            let mut member_use = parent;
            while matches!(
                nodes.kind(nodes.parent_id(member_use)),
                AstKind::ParenthesizedExpression(_)
            ) {
                member_use = nodes.parent_id(member_use);
            }
            let operation = nodes.parent_id(member_use);
            match nodes.kind(operation) {
                AstKind::ArrayAssignmentTarget(_)
                | AstKind::ObjectAssignmentTarget(_)
                | AstKind::AssignmentTargetRest(_)
                | AstKind::AssignmentTargetWithDefault(_)
                | AstKind::AssignmentTargetPropertyProperty(_)
                | AstKind::ForInStatement(_)
                | AstKind::ForOfStatement(_) => {
                    return fail("hash token object has an unmodelled member target");
                }
                AstKind::AssignmentExpression(a)
                    if a.left.span() == nodes.get_node(member_use).span() =>
                {
                    let Some(key) = key else {
                        return fail("hash token object has a dynamic store");
                    };
                    if key == "__proto__" {
                        return fail("hash token object prototype changes");
                    }
                    if key == property {
                        if a.operator != AssignmentOperator::Assign
                            || g.expr_symbol(&a.right) != Some(arg0)
                            || g.owner(operation) != Some(constructor)
                            || !before_read(operation)
                        {
                            return fail("hash token property value is overwritten");
                        }
                        stores.push(operation);
                    }
                }
                kind if matches!(kind, AstKind::UpdateExpression(_))
                    || matches!(kind, AstKind::UnaryExpression(u) if u.operator == oxc_syntax::operator::UnaryOperator::Delete) =>
                {
                    // Unary delete can remove an own property and expose a
                    // prototype value; updates can also coerce an object value.
                    if key.is_none() || key == Some(property) || key == Some("__proto__") {
                        return fail("hash token property is deleted or updated");
                    }
                }
                AstKind::CallExpression(c)
                    if c.callee.span() == nodes.get_node(member_use).span() =>
                {
                    return fail("hash token object has an unresolved method call")
                }
                AstKind::NewExpression(c)
                    if c.callee.span() == nodes.get_node(member_use).span() =>
                {
                    return fail("hash token object has an unresolved constructor call")
                }
                AstKind::TaggedTemplateExpression(_) => {
                    return fail("hash token object has an unresolved tagged call")
                }
                _ => {}
            }
        }
    }
    let [store] = stores.as_slice() else {
        return fail("hash token property needs one reaching public value store");
    };
    // The stored value and the constructor argument must still be the supplied
    // public formal. A later reassignment cannot change a previously copied value.
    let public = wiring
        .public_init
        .as_ref()
        .ok_or(StreamReadinessError("public token entry is missing"))?;
    let entry = g
        .node(public.function.span)
        .ok_or(StreamReadinessError("public token entry source is missing"))?;
    let construction = g
        .node(public.constructor_call_span)
        .ok_or(StreamReadinessError("public token construction is missing"))?;
    let (callee, arguments) = match nodes.kind(construction) {
        AstKind::NewExpression(c) => (&c.callee, &c.arguments),
        AstKind::CallExpression(c) if !c.optional => (&c.callee, &c.arguments),
        _ => return fail("public token construction is unresolved"),
    };
    let formal = g
        .formals(entry)
        .first()
        .copied()
        .flatten()
        .ok_or(StreamReadinessError("public token formal is missing"))?;
    if g.origin(callee) != Some(nodes.get_node(constructor).span().into())
        || g.owner(construction) != Some(entry)
        || arguments
            .first()
            .and_then(|a| a.as_expression())
            .and_then(|e| g.expr_symbol(e))
            != Some(formal)
    {
        return fail("hash token does not originate at the public formal");
    }
    for (symbol, consumed, owner) in [(arg0, *store, constructor), (formal, construction, entry)] {
        if !scopes.symbol_redeclarations(symbol).is_empty()
            || scopes.get_resolved_references(symbol).any(|r| {
                r.is_write()
                    && !(g.owner(r.node_id()) == Some(owner) && g.dominates(consumed, r.node_id()))
            })
        {
            return fail("public token formal value changes before consumption");
        }
        if nodes.iter().any(|n| {
            if !matches!(n.kind(), AstKind::IdentifierReference(id) if id.name == "arguments") {
                return false;
            }
            let mut activation = g.owner(n.id());
            while activation
                .is_some_and(|a| matches!(nodes.kind(a), AstKind::ArrowFunctionExpression(_)))
            {
                activation = activation.and_then(|a| g.owner(a));
            }
            activation == Some(owner)
        }) {
            return fail("public token formal has an unresolved arguments alias");
        }
    }
    Ok(())
}

fn verify_public_entry(
    g: &Graph<'_, '_>,
    wiring: &WiringProperties,
    definition: &StateDefinition,
    context: &InitializationContext,
) -> Result<(SourceSpan, SourceSpan, SourceSpan, SourceSpan)> {
    let public = wiring
        .public_init
        .as_ref()
        .ok_or(StreamReadinessError("public entry missing"))?;
    let entry = g
        .node(public.function.span)
        .ok_or(StreamReadinessError("public entry source missing"))?;
    let init = g
        .node(definition.initializer)
        .ok_or(StreamReadinessError("initializer source missing"))?;
    let constructor = g.owner(init).ok_or(StreamReadinessError(
        "initializer has no constructor activation",
    ))?;
    let constructor_span = SourceSpan::from(g.nodes.get_node(constructor).span());
    let ctor_node = g
        .node(public.constructor_call_span)
        .ok_or(StreamReadinessError("constructor call missing"))?;
    let (callee, args) = match g.nodes.kind(ctor_node) {
        AstKind::NewExpression(n) => (&n.callee, n.arguments.as_slice()),
        AstKind::CallExpression(n) => (&n.callee, n.arguments.as_slice()),
        _ => return fail("constructor is not invoked"),
    };
    if g.origin(callee) != Some(constructor_span) || g.owner(ctor_node) != Some(entry) {
        return fail("constructor belongs to another session");
    }
    let public_formals = g.formals(entry);
    if args
        .first()
        .and_then(|a| a.as_expression())
        .and_then(|e| g.expr_symbol(e))
        != public_formals.first().copied().flatten()
    {
        return fail("constructor token is not public argument zero");
    }
    // There must be one source construction in this entry and no second source
    // call to the same constructor elsewhere. Repeated external initialization
    // remains the explicit epoch prerequisite, never a default true flag.
    let constructions = g
        .nodes
        .iter()
        .filter(|n| match n.kind() {
            AstKind::NewExpression(c) => g.origin(&c.callee) == Some(constructor_span),
            AstKind::CallExpression(c) => g.origin(&c.callee) == Some(constructor_span),
            _ => false,
        })
        .count();
    if constructions != 1 {
        return fail("session construction is ambiguous or duplicated");
    }
    let result = match g.nodes.kind(g.nodes.parent_id(ctor_node)) {
        AstKind::AssignmentExpression(a) if a.operator == AssignmentOperator::Assign => {
            match &a.left {
                AssignmentTarget::AssignmentTargetIdentifier(id) => g.symbol(id),
                _ => None,
            }
        }
        AstKind::VariableDeclarator(d) => match &d.id {
            BindingPattern::BindingIdentifier(b) => b.symbol_id.get(),
            _ => None,
        },
        _ => None,
    }
    .ok_or(StreamReadinessError(
        "constructor result is not a local binding",
    ))?;
    if g.scopes.get_resolved_references(result).any(|r| {
        r.is_write()
            && g.nodes.get_node(r.node_id()).span().start > public.constructor_call_span.end
    }) {
        return fail("session result is overwritten");
    }
    let mut returns = Vec::new();
    for node in g
        .nodes
        .iter()
        .filter(|n| g.owner(n.id()) == Some(constructor))
    {
        if let AstKind::ReturnStatement(r) = node.kind() {
            let Some(mut value) = r.argument.as_ref() else {
                continue;
            };
            for _ in 0..16 {
                let Some(s) = g.expr_symbol(value) else { break };
                let Some(v) = g.value(s) else { break };
                value = v;
            }
            if let Expression::ObjectExpression(object) = value.without_parentheses() {
                for p in &object.properties {
                    if let ObjectPropertyKind::ObjectProperty(p) = p {
                        if !p.computed
                            && p.key.static_name().as_deref() == Some(context.method.as_str())
                            && g.origin(&p.value) == Some(definition.initializer)
                        {
                            returns.push(r.span);
                        }
                    }
                }
            }
        }
    }
    if returns.len() != 1 {
        return fail("constructor does not uniquely return the initializer callback");
    }
    // Confirm that the supplied detail member is the exported public callable,
    // rather than another function with a similarly named property.
    let exported: Vec<_> = g
        .nodes
        .iter()
        .filter_map(|n| match n.kind() {
            AstKind::ObjectProperty(p)
                if !p.computed
                    && p.key.static_name().as_deref() == Some(context.method.as_str())
                    && g.origin(&p.value) == Some(public.function.span) =>
            {
                Some(g.nodes.parent_id(n.id()))
            }
            _ => None,
        })
        .collect();
    let [object] = exported.as_slice() else {
        return fail("supplied detail callback does not select the public entry");
    };
    let object_span = g.nodes.get_node(*object).span();
    let exports: Vec<_> = g
        .nodes
        .iter()
        .filter_map(|n| match n.kind() {
            AstKind::ReturnStatement(r)
                if g.owner(n.id()).is_some_and(|owner| {
                    SourceSpan::from(g.nodes.get_node(owner).span()) == wiring.main.span
                }) =>
            {
                let mut value = r.argument.as_ref()?;
                for _ in 0..16 {
                    let Some(s) = g.expr_symbol(value) else { break };
                    value = g.value(s)?;
                }
                (value.span() == object_span).then_some(r.span)
            }
            _ => None,
        })
        .collect();
    let [public_api_return] = exports.as_slice() else {
        return fail("public API is not the returned source context");
    };
    fn aliases(g: &Graph<'_, '_>, mut s: SymbolId, target: SymbolId) -> bool {
        for _ in 0..16 {
            if s == target {
                return true;
            }
            let Some(next) = g.value(s).and_then(|e| g.expr_symbol(e)) else {
                return false;
            };
            s = next;
        }
        false
    }
    let invocations: Vec<_> = g
        .nodes
        .iter()
        .filter_map(|n| match n.kind() {
            AstKind::CallExpression(c) if g.owner(n.id()) == Some(entry) => {
                let Expression::StaticMemberExpression(m) = c.callee.without_parentheses() else {
                    return None;
                };
                (m.property.name.as_str() == context.method.as_str()
                    && g.expr_symbol(&m.object)
                        .is_some_and(|s| aliases(g, s, result)))
                .then_some((n.id(), c))
            }
            _ => None,
        })
        .collect();
    let [(invocation, call)] = invocations.as_slice() else {
        return fail("returned initializer is not invoked exactly once");
    };
    if call.optional
        || call.span.start <= public.constructor_call_span.end
        || !g.whole_statement(*invocation)
    {
        return fail("initializer invocation is conditional or reordered");
    }
    if !g.dominates(ctor_node, *invocation) {
        return fail("session construction does not dominate initialization");
    }
    // No other method of this instance may start collection first, and no
    // unknown callback may receive it before initialization.
    let mut pending = vec![result];
    let mut seen = Vec::new();
    while let Some(symbol) = pending.pop() {
        if seen.contains(&symbol) {
            return fail("session alias cycle");
        };
        seen.push(symbol);
        for r in g.scopes.get_resolved_references(symbol) {
            let span = g.nodes.get_node(r.node_id()).span();
            if r.is_write() && span.start > public.constructor_call_span.end {
                return fail("session alias is overwritten");
            }
            if public.constructor_call_span.end < span.start && span.start < call.span.start {
                if let AstKind::VariableDeclarator(d) = g.nodes.kind(g.nodes.parent_id(r.node_id()))
                {
                    if d.init.as_ref().is_some_and(|e| e.span() == span) {
                        if let BindingPattern::BindingIdentifier(b) = &d.id {
                            if let Some(s) = b.symbol_id.get() {
                                pending.push(s);
                                continue;
                            }
                        }
                    }
                }
                return fail("session escapes or starts before initialization");
            }
        }
    }
    // Bind the actual hashed property to argument zero of this same constructor.
    let input = g
        .nodes
        .iter()
        .find_map(|n| match n.kind() {
            AstKind::AssignmentExpression(a)
                if definition.assignment.start <= a.span.start
                    && a.span.end <= definition.assignment.end =>
            {
                if let Expression::CallExpression(c) = a.right.without_parentheses() {
                    c.arguments.first().and_then(|a| a.as_expression())
                } else {
                    None
                }
            }
            AstKind::VariableDeclarator(d)
                if definition.assignment.start <= d.span.start
                    && d.span.end <= definition.assignment.end =>
            {
                if let Some(Expression::CallExpression(c)) =
                    d.init.as_ref().map(Expression::without_parentheses)
                {
                    c.arguments.first().and_then(|a| a.as_expression())
                } else {
                    None
                }
            }
            _ => None,
        })
        .ok_or(StreamReadinessError("hash argument source missing"))?;
    let Expression::StaticMemberExpression(member) = input.without_parentheses() else {
        return fail("token input is not a context property");
    };
    let root = g
        .object_root(&member.object, 0)
        .ok_or(StreamReadinessError("token context origin is ambiguous"))?;
    if g.owner(g.scopes.symbol_declaration(root)) != Some(constructor) {
        return fail("token context belongs to another constructor");
    }
    let return_node = g
        .node(returns[0].into())
        .ok_or(StreamReadinessError("constructor return source missing"))?;
    let mut current = g
        .expr_symbol(&member.object)
        .ok_or(StreamReadinessError("context is not local"))?;
    for _ in 0..16 {
        let declaration = g.scopes.symbol_declaration(current);
        let definition = if matches!(g.nodes.kind(declaration),AstKind::VariableDeclarator(d) if d.init.is_some())
        {
            declaration
        } else {
            let writes: Vec<_> = g
                .scopes
                .get_resolved_references(current)
                .filter(|r| r.is_write())
                .collect();
            let [write] = writes.as_slice() else {
                return fail("context alias has ambiguous writes");
            };
            g.nodes.parent_id(write.node_id())
        };
        if !g.dominates(definition, return_node) {
            return fail("context initialization does not dominate returned callback");
        }
        if current == root {
            break;
        }
        current = g
            .value(current)
            .and_then(|e| g.expr_symbol(e))
            .ok_or(StreamReadinessError("context alias changes value"))?;
    }
    // The shared state-definition gate has already proved the reaching value,
    // including computed stores, aliases and unresolved object consumers.
    Ok((
        public.function.span,
        public.constructor_call_span,
        call.span.into(),
        (*public_api_return).into(),
    ))
}

/// Follow every direct call of a nested source helper back to its actual
/// registered activation. Taking a helper value is not an invocation; unknown
/// consumers, mutable bindings and recursive/cyclic carriers stay refused.
fn verify_activation(
    g: &Graph<'_, '_>,
    wiring: &WiringProperties,
    instance: NodeId,
    emission: NodeId,
    definition: &StateDefinition,
    path: &mut Vec<SourceSpan>,
    visiting: &mut Vec<NodeId>,
) -> Result<()> {
    if g.after(instance, definition, &mut vec![], path) {
        return Ok(());
    }
    let mut registration_path = Vec::new();
    let error = match verify_registered_activation(
        g,
        wiring,
        instance,
        emission,
        definition,
        &mut registration_path,
    ) {
        Ok(()) => {
            path.extend(registration_path);
            return Ok(());
        }
        Err(error) => error,
    };
    let owner = g.owner(instance).ok_or_else(|| error.clone())?;
    if visiting.len() >= 32 || visiting.contains(&owner) {
        return fail("activation carrier is cyclic");
    }
    // An inline function value is allocated while its enclosing activation
    // evaluates this argument/callee. If THAT activation is already proven
    // initialized, no consumer can invoke the newly created closure earlier.
    // This proves earliest exposure, not scheduler order or consumer behavior.
    // Named/aliased callbacks do not inherit this expression-lifetime proof.
    let parent = g.nodes.parent_id(owner);
    let inline = matches!(g.nodes.kind(owner), AstKind::ArrowFunctionExpression(_))
        || matches!(g.nodes.kind(owner),AstKind::Function(f) if f.r#type==FunctionType::FunctionExpression);
    if inline
        && matches!(
            g.nodes.kind(parent),
            AstKind::CallExpression(_) | AstKind::NewExpression(_)
        )
    {
        visiting.push(owner);
        let result = verify_activation(g, wiring, owner, owner, definition, path, visiting);
        visiting.pop();
        if result.is_ok() {
            path.push(g.nodes.get_node(owner).span().into());
            return Ok(());
        }
    }
    // A nested declaration is instantiated when its enclosing activation
    // enters, regardless of declaration text position. Prove that activation's
    // ENTRY is initialized; a declaration written below the hash is not enough.
    if matches!(g.nodes.kind(owner), AstKind::Function(function) if function.r#type == FunctionType::FunctionDeclaration
        && function.id.as_ref().and_then(|id| id.symbol_id.get()).is_some_and(|symbol|
            g.scopes.symbol_redeclarations(symbol).is_empty() && !g.scopes.get_resolved_references(symbol).any(|reference| reference.is_write())))
    {
        if let Some(parent_activation) = g.owner(owner) {
            let body = match g.nodes.kind(parent_activation) {
                AstKind::Function(function) => function.body.as_ref().map(|body| body.span),
                AstKind::ArrowFunctionExpression(function) => Some(function.body.span),
                _ => None,
            };
            if let Some(entry) = body.and_then(|span| {
                g.nodes
                    .iter()
                    .find(|node| {
                        matches!(node.kind(), AstKind::FunctionBody(_)) && node.span() == span
                    })
                    .map(|node| node.id())
            }) {
                visiting.push(owner);
                let result = verify_activation(g, wiring, entry, entry, definition, path, visiting);
                visiting.pop();
                if result.is_ok() {
                    path.push(g.nodes.get_node(owner).span().into());
                    return Ok(());
                }
            }
        }
    }
    let binding = match g.nodes.kind(owner) {
        AstKind::Function(function) if function.r#type == FunctionType::FunctionDeclaration => {
            function.id.as_ref().and_then(|id| id.symbol_id.get())
        }
        _ => None,
    }
    .ok_or_else(|| error.clone())?;
    if !g.scopes.symbol_redeclarations(binding).is_empty() {
        return fail("activation helper is redeclared");
    }
    let mut pending = vec![binding];
    let mut aliases = Vec::new();
    let mut calls = Vec::new();
    while let Some(symbol) = pending.pop() {
        if aliases.contains(&symbol) {
            return fail("activation alias is cyclic");
        }
        aliases.push(symbol);
        for reference in g.scopes.get_resolved_references(symbol) {
            if reference.is_write() {
                return fail("activation helper is overwritten");
            }
            let id = reference.node_id();
            let parent = g.nodes.parent_id(id);
            match g.nodes.kind(parent) {
                AstKind::VariableDeclarator(d)
                    if d.init
                        .as_ref()
                        .is_some_and(|e| e.span() == g.nodes.get_node(id).span()) =>
                {
                    let BindingPattern::BindingIdentifier(alias) = &d.id else {
                        return Err(error);
                    };
                    pending.push(alias.symbol_id.get().ok_or_else(|| error.clone())?);
                }
                AstKind::CallExpression(call)
                    if call.callee.span() == g.nodes.get_node(id).span() =>
                {
                    calls.push(parent)
                }
                _ => return Err(error),
            }
        }
    }
    if calls.is_empty() {
        return Err(error);
    }
    visiting.push(owner);
    for call in calls {
        verify_activation(g, wiring, call, call, definition, path, visiting)?;
        path.push(g.nodes.get_node(call).span().into());
    }
    visiting.pop();
    Ok(())
}

// Boolean guards inspect callback presence without invoking or exposing it.
// A short-circuit value is safe only when its enclosing use discards the value
// or converts it to a boolean; returning/assigning the callback stays refused.
fn observes_callability(g: &Graph<'_, '_>, mut id: NodeId) -> bool {
    loop {
        let p = g.nodes.parent_id(id);
        match g.nodes.kind(p) {
            AstKind::UnaryExpression(u)
                if matches!(
                    u.operator,
                    oxc_syntax::operator::UnaryOperator::Typeof
                        | oxc_syntax::operator::UnaryOperator::LogicalNot
                ) =>
            {
                return true
            }
            AstKind::IfStatement(statement)
                if statement.test.span() == g.nodes.get_node(id).span() =>
            {
                return true
            }
            AstKind::ConditionalExpression(expression)
                if expression.test.span() == g.nodes.get_node(id).span() =>
            {
                return true
            }
            AstKind::LogicalExpression(_) => id = p,
            AstKind::ExpressionStatement(_) if p != id => return true,
            _ => return false,
        }
    }
}

/// Named local callbacks and immutable aliases retain their value identity.
/// Every reference must lead to the same single registrar argument; unknown
/// consumers, assignments, duplicate registration and cyclic aliases refuse.
fn callback_registration(g: &Graph<'_, '_>, callback: NodeId) -> Result<NodeId> {
    let parent = g.nodes.parent_id(callback);
    if let AstKind::CallExpression(call) = g.nodes.kind(parent) {
        if call
            .arguments
            .first()
            .and_then(|a| a.as_expression())
            .is_some_and(|e| e.span() == g.nodes.get_node(callback).span())
        {
            return Ok(parent);
        }
        return fail("callback is not the registrar's first argument");
    }

    let binding = match g.nodes.kind(callback) {
        AstKind::Function(function) if function.r#type == FunctionType::FunctionDeclaration => {
            function.id.as_ref().and_then(|id| id.symbol_id.get())
        }
        _ => match g.nodes.kind(parent) {
            AstKind::VariableDeclarator(d) => match &d.id {
                BindingPattern::BindingIdentifier(id) => id.symbol_id.get(),
                _ => None,
            },
            _ => None,
        },
    }
    .ok_or(StreamReadinessError(
        "callback escapes through an unknown carrier",
    ))?;
    let mut pending = vec![binding];
    let mut seen = Vec::new();
    let mut registration = None;
    while let Some(symbol) = pending.pop() {
        if seen.contains(&symbol) || !g.scopes.symbol_redeclarations(symbol).is_empty() {
            return fail("callback alias identity is ambiguous");
        }
        seen.push(symbol);
        for reference in g.scopes.get_resolved_references(symbol) {
            if reference.is_write() {
                return fail("registered callback binding is overwritten");
            }
            let id = reference.node_id();
            let parent = g.nodes.parent_id(id);
            match g.nodes.kind(parent) {
                AstKind::VariableDeclarator(d)
                    if d.init
                        .as_ref()
                        .is_some_and(|e| e.span() == g.nodes.get_node(id).span()) =>
                {
                    let BindingPattern::BindingIdentifier(alias) = &d.id else {
                        return fail("callback alias is destructured");
                    };
                    pending.push(
                        alias
                            .symbol_id
                            .get()
                            .ok_or(StreamReadinessError("callback alias binding missing"))?,
                    );
                }
                AstKind::CallExpression(call)
                    if call
                        .arguments
                        .first()
                        .and_then(|a| a.as_expression())
                        .is_some_and(|e| e.span() == g.nodes.get_node(id).span()) =>
                {
                    if registration.is_some_and(|existing| existing != parent) {
                        return fail("callback has multiple registrations");
                    }
                    registration = Some(parent);
                }
                _ if observes_callability(g, id) => {}
                _ => return fail("callback escapes through an unknown carrier"),
            }
        }
    }
    registration.ok_or(StreamReadinessError("callback registration missing"))
}

fn verify_registered_activation(
    g: &Graph<'_, '_>,
    wiring: &WiringProperties,
    instance: NodeId,
    emission: NodeId,
    definition: &StateDefinition,
    path: &mut Vec<SourceSpan>,
) -> Result<()> {
    let callback = g
        .owner(instance)
        .ok_or(StreamReadinessError("instance callback missing"))?;
    if g.owner(emission) != Some(callback) {
        return fail("emission callback differs");
    }
    let callback_span = g.nodes.get_node(callback).span();
    // The callback is an argument to a registrar of its owning producer.
    let registration = callback_registration(g, callback)?;

    let AstKind::CallExpression(registration_call) = g.nodes.kind(registration) else {
        return fail("callback escapes through an unknown carrier");
    };
    if registration_call
        .arguments
        .first()
        .and_then(|a| a.as_expression())
        .is_none_or(|e| {
            e.span() != callback_span
                && !matches!(e.without_parentheses(), Expression::Identifier(_))
        })
    {
        return fail("callback is not the registrar's first argument");
    }
    let group = g
        .owner(registration)
        .ok_or(StreamReadinessError("producer missing"))?;
    let registrar = g
        .expr_symbol(&registration_call.callee)
        .ok_or(StreamReadinessError("registrar is not a producer formal"))?;
    let index = g
        .formals(group)
        .iter()
        .position(|s| *s == Some(registrar))
        .ok_or(StreamReadinessError("registrar is not a producer argument"))?;
    let group_span = SourceSpan::from(g.nodes.get_node(group).span());
    let route = wiring
        .groups
        .iter()
        .find(|r| r.function.span == group_span)
        .ok_or(StreamReadinessError("producer route missing"))?;
    let invocation = route
        .invocation
        .as_ref()
        .ok_or(StreamReadinessError("producer invocation missing"))?;
    let invoke = g
        .call(invocation.span)
        .ok_or(StreamReadinessError("producer invocation source missing"))?;
    let coordinator = g
        .owner(
            g.node(invocation.span)
                .ok_or(StreamReadinessError("invocation node missing"))?,
        )
        .ok_or(StreamReadinessError("coordinator missing"))?;
    let body_formal = g.expr_symbol(&invoke.callee).ok_or(StreamReadinessError(
        "producer invocation is not a formal call",
    ))?;
    if g.scopes
        .get_resolved_references(body_formal)
        .any(|r| r.is_write() || g.nodes.get_node(r.node_id()).span() != invoke.callee.span())
    {
        return fail("producer callable escapes to an unknown consumer");
    }
    let body_index = g
        .formals(coordinator)
        .iter()
        .position(|s| *s == Some(body_formal))
        .ok_or(StreamReadinessError("producer callable is not forwarded"))?;
    let registrar_fn = invoke
        .arguments
        .get(index)
        .and_then(|a| a.as_expression())
        .ok_or(StreamReadinessError("registrar source argument missing"))?;
    let registrar_node = g
        .node(registrar_fn.span().into())
        .ok_or(StreamReadinessError("registrar node missing"))?;
    let arg0 = g
        .formals(registrar_node)
        .first()
        .copied()
        .flatten()
        .ok_or(StreamReadinessError("registrar callback argument missing"))?;
    let stores: Vec<_> = g
        .nodes
        .iter()
        .filter_map(|n| match n.kind() {
            AstKind::AssignmentExpression(a)
                if g.owner(n.id()) == Some(registrar_node)
                    && a.operator == AssignmentOperator::Assign
                    && g.expr_symbol(&a.right) == Some(arg0) =>
            {
                match &a.left {
                    AssignmentTarget::AssignmentTargetIdentifier(id) => g.symbol(id),
                    _ => None,
                }
            }
            _ => None,
        })
        .collect();
    let [stored] = stores.as_slice() else {
        return fail("registrar does not uniquely store its callback");
    };
    // A stored callback must actually be consumed by this coordinator. Prove
    // the source call/argument, not a LifecycleRole label or a boolean fact.
    let mut consumed = 0;
    let mut pending = vec![*stored];
    let mut seen = Vec::new();
    while let Some(current) = pending.pop() {
        if seen.contains(&current) {
            return fail("stored callback alias is cyclic");
        }
        seen.push(current);
        for r in g
            .scopes
            .get_resolved_references(current)
            .filter(|r| r.is_read())
        {
            let id = r.node_id();
            let p = g.nodes.parent_id(id);
            if let AstKind::CallExpression(c) = g.nodes.kind(p) {
                if c.span.start < invoke.span.end {
                    return fail("callback invoked before registration");
                }
                let direct = c.callee.span() == g.nodes.get_node(id).span();
                let helper = if !direct {
                    g.origin(&c.callee).and_then(|s|g.function(s)).is_some_and(|f|{
                let Some(symbol)=f.params.items.first().and_then(|p|match &p.pattern{BindingPattern::BindingIdentifier(b)=>b.symbol_id.get(),_=>None})else{return false};
                g.nodes.iter().any(|n|matches!(n.kind(),AstKind::CallExpression(call) if f.span.start<=call.span.start&&call.span.end<=f.span.end&&g.expr_symbol(&call.callee)==Some(symbol)))
            })
                } else {
                    false
                };
                if !direct && !helper {
                    return fail("stored callback escapes to an unknown consumer");
                }
                consumed += 1;
            } else if let AstKind::VariableDeclarator(declaration) = g.nodes.kind(p) {
                if declaration.span.start < invoke.span.end
                    || g.owner(id) != Some(coordinator)
                    || declaration
                        .init
                        .as_ref()
                        .is_none_or(|value| value.span() != g.nodes.get_node(id).span())
                {
                    return fail("stored callback alias is not dominated by registration");
                }
                let BindingPattern::BindingIdentifier(alias) = &declaration.id else {
                    return fail("stored callback alias is destructured");
                };
                let alias = alias.symbol_id.get().ok_or(StreamReadinessError(
                    "stored callback alias binding missing",
                ))?;
                if !g.scopes.symbol_redeclarations(alias).is_empty()
                    || g.scopes
                        .get_resolved_references(alias)
                        .any(|reference| reference.is_write())
                {
                    return fail("stored callback alias is mutable");
                }
                if g.scopes.get_resolved_references(current).any(|reference| {
                    reference.is_write() && {
                        let span = g.nodes.get_node(reference.node_id()).span();
                        invoke.span.end < span.start && span.start < declaration.span.start
                    }
                }) {
                    return fail("stored callback changes before alias capture");
                }
                path.push(declaration.span.into());
                pending.push(alias);
            } else if observes_callability(g, id) {
                // A type guard observes callability without invoking or exposing it.
                continue;
            } else if matches!(g.nodes.kind(p), AstKind::BinaryExpression(b) if matches!(b.operator,
            BinaryOperator::Equality | BinaryOperator::Inequality | BinaryOperator::StrictEquality | BinaryOperator::StrictInequality)
            && (nullish_guard(&b.left,g) || nullish_guard(&b.right,g)))
            {
                continue;
            } else if matches!(g.nodes.kind(p), AstKind::AssignmentExpression(_)) {
                /* cleanup resets are writes, rejected below if they read */
                return fail("stored callback is aliased");
            } else {
                return fail("stored callback has an unsupported read");
            }
        }
    }
    if consumed == 0 {
        return fail("stored callback has no source invocation");
    }
    // The producer is a member of one immutable source table. Its concrete
    // coordinator activation must receive that table's element from the same
    // forEach callback, and that callback must be exposed after the hash.
    let group_fn = g
        .function(group_span)
        .ok_or(StreamReadinessError("producer is not a function"))?;
    let group_symbol = group_fn
        .id
        .as_ref()
        .and_then(|i| i.symbol_id.get())
        .ok_or(StreamReadinessError("producer binding missing"))?;
    let mut table = None;
    for r in g.scopes.get_resolved_references(group_symbol) {
        if r.is_write() {
            return fail("producer is overwritten");
        }
        let p = g.nodes.parent_id(r.node_id());
        let AstKind::ArrayExpression(array) = g.nodes.kind(p) else {
            return fail("producer escapes outside its source table");
        };
        let parent = g.nodes.parent_id(p);
        let symbol = match g.nodes.kind(parent) {
            AstKind::VariableDeclarator(d) => match &d.id {
                BindingPattern::BindingIdentifier(b) => b.symbol_id.get(),
                _ => None,
            },
            AstKind::AssignmentExpression(a) if a.operator == AssignmentOperator::Assign => {
                match &a.left {
                    AssignmentTarget::AssignmentTargetIdentifier(id) => g.symbol(id),
                    _ => None,
                }
            }
            _ => None,
        }
        .ok_or(StreamReadinessError("producer table is not a local"))?;
        if g.value(symbol).is_none_or(|e| e.span() != array.span)
            || table.is_some_and(|t| t != symbol)
        {
            return fail("producer table origin is ambiguous");
        };
        table = Some(symbol);
    }
    let table = table.ok_or(StreamReadinessError("producer table missing"))?;
    let coord_span = SourceSpan::from(g.nodes.get_node(coordinator).span());
    let mut activations = 0;
    for r in g.scopes.get_resolved_references(table) {
        if r.is_write() {
            continue;
        }
        let p = g.nodes.parent_id(r.node_id());
        let AstKind::StaticMemberExpression(m) = g.nodes.kind(p) else {
            return fail("producer table escapes");
        };
        if m.property.name != "forEach" {
            return fail("producer table has unsupported uses");
        }
        let parent = g.nodes.parent_id(p);
        let AstKind::CallExpression(for_each) = g.nodes.kind(parent) else {
            return fail("producer table iteration missing");
        };
        let fn_expr = for_each
            .arguments
            .first()
            .and_then(|a| a.as_expression())
            .ok_or(StreamReadinessError("iteration callback missing"))?;
        let fn_id = g
            .node(fn_expr.span().into())
            .ok_or(StreamReadinessError("iteration callback source missing"))?;
        let element = g
            .formals(fn_id)
            .first()
            .copied()
            .flatten()
            .ok_or(StreamReadinessError("iteration element missing"))?;
        if !g.after(fn_id, definition, &mut vec![], path) {
            return fail("producer table can activate before initialization");
        }
        let found: Vec<_> = g
            .nodes
            .iter()
            .filter_map(|n| match n.kind() {
                AstKind::CallExpression(c)
                    if g.owner(n.id()) == Some(fn_id)
                        && g.origin(&c.callee) == Some(coord_span)
                        && c.arguments
                            .get(body_index)
                            .and_then(|a| a.as_expression())
                            .and_then(|e| g.expr_symbol(e))
                            == Some(element) =>
                {
                    Some(c.span)
                }
                _ => None,
            })
            .collect();
        if found.len() != 1 {
            return fail("coordinator does not receive the same table element exactly once");
        }
        path.push(found[0].into());
        activations += 1;
    }
    if activations != 1 {
        return fail("producer activation is missing or duplicated");
    }
    path.push(registration_call.span.into());
    path.push(invoke.span.into());
    Ok(())
}

fn nullish_guard(expression: &Expression<'_>, g: &Graph<'_, '_>) -> bool {
    match expression.without_parentheses() {
        Expression::NullLiteral(_) => true,
        Expression::UnaryExpression(unary) => {
            unary.operator == oxc_syntax::operator::UnaryOperator::Void
                && matches!(unary.argument.without_parentheses(), Expression::NumericLiteral(number) if number.value == 0.0)
        }
        Expression::Identifier(id) => id.name == "undefined" && g.symbol(id).is_none(),
        _ => false,
    }
}
