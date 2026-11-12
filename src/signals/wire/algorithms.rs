//! Whole-body alpha-equivalence against reviewed generic native algorithms.
//! Recognition properties select candidates, never certify their semantics.
use crate::signals::{
    extract::{encoding::EncodingProperties, wiring::WiringProperties},
    schema::SourceSpan,
};
use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstKind};
use oxc_ast_visit::{walk, Visit};
use oxc_codegen::Codegen;
use oxc_parser::Parser;
use oxc_semantic::{Scoping, SemanticBuilder};
use oxc_span::{GetSpan, SourceType};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AlgorithmProof {
    pub codec: Option<String>,
    pub span: Option<SourceSpan>,
    pub sha256: Option<String>,
    pub reason: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AlgorithmReport {
    pub hash: AlgorithmProof,
    pub stream: AlgorithmProof,
    pub integer: AlgorithmProof,
    /// The disjoint float marker, independently of the unproved float body.
    pub float_prefix: AlgorithmProof,
    pub string: AlgorithmProof,
    pub float: AlgorithmProof,
}
pub(super) fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[derive(Default)]
struct Ids {
    references: Vec<(oxc_span::Span, String, Option<u32>, bool)>,
    bindings: Vec<(oxc_span::Span, u32)>,
    deletes: Vec<(usize, usize, String)>,

    invalid: bool,
    lifted: Vec<(u32, u32)>,
}
struct Scan<'s> {
    ids: Ids,
    scopes: &'s Scoping,
}
// Only inert entry expressions may participate in declaration lifting.
// In particular calling a hoisted closure can observe a later let's TDZ even
// when every textual reference follows the declaration. Object.create(null)
// is the reviewed table allocation (its builtin origin is checked by certify).
fn inert_entry(e: &Expression<'_>) -> bool {
    match e.without_parentheses() {
        Expression::Identifier(_)
        | Expression::NumericLiteral(_)
        | Expression::BooleanLiteral(_)
        | Expression::NullLiteral(_)
        | Expression::StringLiteral(_) => true,
        // Even arithmetic on an identifier can invoke user conversion hooks.
        // No reviewed lifted initializer needs coercion or a property read.
        Expression::AssignmentExpression(e) => {
            matches!(e.left, AssignmentTarget::AssignmentTargetIdentifier(_))
                && inert_entry(&e.right)
        }
        Expression::CallExpression(c) => {
            !c.optional
                && c.arguments.len() == 1
                && matches!(
                    c.arguments[0].as_expression(),
                    Some(Expression::NullLiteral(_))
                )
                && matches!(&c.callee, Expression::StaticMemberExpression(m)
                if m.property.name == "create" && !m.optional
                && matches!(&m.object, Expression::Identifier(id) if id.name == "Object"))
        }
        _ => false,
    }
}

impl<'a> Visit<'a> for Scan<'_> {
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        let sym = id
            .reference_id
            .get()
            .and_then(|r| self.scopes.get_reference(r).symbol_id())
            .map(|s| s.index() as u32);
        let read = id
            .reference_id
            .get()
            .is_some_and(|r| self.scopes.get_reference(r).is_read());
        self.ids
            .references
            .push((id.span, id.name.to_string(), sym, read));
    }
    fn visit_binding_identifier(&mut self, id: &BindingIdentifier<'a>) {
        if let Some(s) = id.symbol_id.get() {
            self.ids.bindings.push((id.span, s.index() as u32));
        }
    }
    fn visit_function(&mut self, f: &Function<'a>, flags: oxc_syntax::scope::ScopeFlags) {
        if let Some(body) = &f.body {
            for (index, statement) in body.statements.iter().enumerate() {
                if let Statement::VariableDeclaration(v) = statement {
                    for d in &v.declarations {
                        self.ids.invalid |= d.init.as_ref().is_some_and(|e| !inert_entry(e));
                        if let BindingPattern::BindingIdentifier(b) = &d.id {
                            if let Some(symbol) = b.symbol_id.get() {
                                // At function entry mutable locals may be lifted only
                                // when neither TDZ/hoisting nor redeclaration is observable.
                                self.ids.invalid |=
                                    !self.scopes.symbol_redeclarations(symbol).is_empty();
                                self.ids.lifted.push((symbol.index() as u32, d.span.end));
                            }
                        }
                    }
                }
                match statement {
                    Statement::VariableDeclaration(v)
                        if v.kind != VariableDeclarationKind::Const
                            && v.declarations.iter().all(|d| {
                                d.init.is_none()
                                    && matches!(d.id, BindingPattern::BindingIdentifier(_))
                            }) =>
                    {
                        self.ids.deletes.push((
                            v.span.start as usize,
                            v.span.end as usize,
                            String::new(),
                        ))
                    }
                    Statement::VariableDeclaration(v)
                        if v.kind != VariableDeclarationKind::Const
                            && v.declarations.iter().all(|d| {
                                d.init.is_some()
                                    && matches!(d.id, BindingPattern::BindingIdentifier(_))
                            }) =>
                    {
                        self.ids.deletes.push((
                            v.span.start as usize,
                            v.declarations[0].span.start as usize,
                            String::new(),
                        ));
                    }
                    Statement::ExpressionStatement(e)
                        if matches!(e.expression, Expression::AssignmentExpression(_)) =>
                    {
                        if body.statements[index + 1..]
                            .iter()
                            .any(|s| matches!(s, Statement::VariableDeclaration(_)))
                        {
                            self.ids.invalid |= !inert_entry(&e.expression);
                        }
                    }
                    _ => break,
                }
            }
        }
        walk::walk_function(self, f, flags);
    }
}

// Canonicalization is scope-aware. Properties, operators, constants, statement
// order and globals survive; only bound/captured identifier spelling changes.
fn canonical(text: &str, integer: bool) -> Option<String> {
    let arena = Allocator::default();
    let parsed = Parser::new(&arena, text, SourceType::mjs()).parse();
    if !parsed.errors.is_empty() {
        return None;
    }
    let built = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    if !built.errors.is_empty() {
        return None;
    }
    let semantic = built.semantic;
    let mut scan = Scan {
        ids: Ids::default(),
        scopes: semantic.scoping(),
    };
    scan.visit_program(&parsed.program);
    if scan.ids.invalid {
        return None;
    }
    // Leading mutable locals have equivalent initialization only if no
    // reference observes their TDZ/hoisting or reads their own initializer.
    if scan.ids.lifted.iter().any(|(symbol, end)| {
        scan.ids
            .references
            .iter()
            .any(|(span, _, s, _)| *s == Some(*symbol) && span.start < *end)
    }) {
        return None;
    }
    let Statement::FunctionDeclaration(function) = parsed.program.body.first()? else {
        return None;
    };
    let body = function.body.as_ref()?;
    if integer {
        let branch = body.statements.iter().find_map(|s| {
            if let Statement::IfStatement(i) = s {
                Some(i)
            } else {
                None
            }
        })?;
        let alt = branch.alternate.as_ref()?.span();
        scan.ids
            .deletes
            .push((alt.start as usize, alt.end as usize, "{throw 0;}".into()));
    }
    let hidden = |span: oxc_span::Span| {
        scan.ids
            .deletes
            .iter()
            .any(|(s, e, _)| *s <= span.start as usize && span.end as usize <= *e)
    };
    let mut local = HashMap::new();
    let mut external = HashMap::new();
    let mut edits = Vec::new();
    for (span, _, symbol, read) in &scan.ids.references {
        if *read && !hidden(*span) {
            if let Some(symbol) = symbol {
                let next = local.len();
                local.entry(*symbol).or_insert_with(|| format!("v{next}"));
            }
        }
    }
    for (span, name, symbol, _) in &scan.ids.references {
        if hidden(*span) {
            continue;
        }
        let replacement = if let Some(symbol) = symbol {
            let next = local.len();
            local
                .entry(*symbol)
                .or_insert_with(|| format!("v{next}"))
                .clone()
        } else if [
            "Number",
            "Math",
            "Object",
            "ArrayBuffer",
            "Int8Array",
            "Int32Array",
            "String",
            "Array",
            "Error",
            "TypeError",
        ]
        .contains(&name.as_str())
        {
            name.clone()
        } else {
            let next = external.len();
            external
                .entry(name.clone())
                .or_insert_with(|| format!("e{next}"))
                .clone()
        };
        edits.push((span.start as usize, span.end as usize, replacement));
    }
    for (span, symbol) in &scan.ids.bindings {
        if hidden(*span) {
            continue;
        }
        let next = local.len();
        let name = local
            .entry(*symbol)
            .or_insert_with(|| format!("v{next}"))
            .clone();
        edits.push((span.start as usize, span.end as usize, name));
    }
    edits.extend(scan.ids.deletes);
    edits.sort_by_key(|(s, _, _)| *s);
    let mut out = String::new();
    let mut end = 0;
    for (s, e, value) in edits {
        if s < end {
            return None;
        }
        out.push_str(text.get(end..s)?);
        out.push_str(&value);
        end = e;
    }
    out.push_str(text.get(end..)?);
    let arena = Allocator::default();
    let parsed = Parser::new(&arena, &out, SourceType::mjs()).parse();
    if !parsed.errors.is_empty() {
        return None;
    }
    Some(Codegen::new().build(&parsed.program).code)
}

pub(super) fn proof(
    candidate: Option<(SourceSpan, &str)>,
    reference: &str,
    codec: &str,
    integer: bool,
) -> AlgorithmProof {
    let mut result = AlgorithmProof {
        codec: None,
        span: candidate.map(|c| c.0),
        sha256: candidate.map(|c| digest(c.1.as_bytes())),
        reason: Some("whole helper algorithm is not certified".into()),
    };
    if let Some((_, text)) = candidate {
        if canonical(text, integer)
            .zip(canonical(reference, integer))
            .is_some_and(|(a, b)| a == b)
        {
            result.codec = Some(codec.into());
            result.reason = None;
        }
    }
    result
}
pub(super) fn float_prefix_proof(
    candidate: Option<(SourceSpan, &str)>,
    reference: &str,
) -> AlgorithmProof {
    fn prefix(text: &str) -> Option<String> {
        let arena = Allocator::default();
        let parsed = Parser::new(&arena, text, SourceType::mjs()).parse();
        if !parsed.errors.is_empty() {
            return None;
        }
        let Statement::FunctionDeclaration(f) = parsed.program.body.first()? else {
            return None;
        };
        let branch = f.body.as_ref()?.statements.iter().find_map(|s| {
            if let Statement::IfStatement(i) = s {
                Some(i)
            } else {
                None
            }
        })?;
        let Statement::BlockStatement(alt) = branch.alternate.as_ref()? else {
            return None;
        };
        let end = alt.body.get(1)?.span().end as usize;
        let consequent = branch.consequent.span();
        let mut out = String::new();
        out.push_str(text.get(..consequent.start as usize)?);
        out.push_str("{throw 0;}");
        out.push_str(text.get(consequent.end as usize..end)?);
        out.push_str(text.get(alt.span.end as usize - 1..)?);
        canonical(&out, false)
    }
    let mut result = AlgorithmProof {
        codec: None,
        span: candidate.map(|c| c.0),
        sha256: candidate.map(|c| digest(c.1.as_bytes())),
        reason: Some("float branch prefix is not certified".into()),
    };
    if candidate.is_some_and(|(_, text)| {
        prefix(text)
            .zip(prefix(reference))
            .is_some_and(|(a, b)| a == b)
    }) {
        result.codec = Some("float-marker-128/1".into());
        result.reason = None;
    }
    result
}
fn unsupported(reason: &str) -> AlgorithmProof {
    AlgorithmProof {
        codec: None,
        span: None,
        sha256: None,
        reason: Some(reason.into()),
    }
}

pub(super) fn certify(
    source: &str,
    encoding: &EncodingProperties,
    wiring: &WiringProperties,
) -> AlgorithmReport {
    let candidate = |span: SourceSpan| {
        source
            .get(span.start as usize..span.end as usize)
            .map(|s| (span, s))
    };
    let mut hash = proof(
        encoding
            .seed_link
            .hash
            .as_ref()
            .and_then(|h| candidate(h.span)),
        include_str!("reference/hash.js"),
        "paired-utf16-h31/1",
        false,
    );
    let mut stream = proof(
        encoding
            .byte_stream
            .as_ref()
            .and_then(|h| candidate(h.factory.span)),
        include_str!("reference/stream.js"),
        "sum-rot26-9-13-le-burn4/1",
        false,
    );
    let mut integer = proof(
        encoding
            .number
            .as_ref()
            .and_then(|h| candidate(h.function.span)),
        include_str!("reference/number.js"),
        "sign-magnitude-5-7/1",
        true,
    );
    let float_prefix = float_prefix_proof(
        encoding
            .number
            .as_ref()
            .and_then(|h| candidate(h.function.span)),
        include_str!("reference/number.js"),
    );
    if integer.codec.is_some() && !unshadowed(source, integer.span.unwrap(), &["Number"]) {
        integer.codec = None;
        integer.reason = Some("number conversion dependency is shadowed".into());
    }
    let token = wiring.public_init.as_ref().and_then(|p| p.token.as_ref());
    if !token.is_some_and(|t| {
        t.public_init_argument == Some(0)
            && t.constructor_argument == Some(0)
            && t.unresolved.is_empty()
    }) {
        hash.codec = None;
        hash.reason = Some("public initializer token/hash link is not proven".into());
        stream.codec = None;
        stream.reason = Some("token state/stream link is not proven".into());
    }
    if stream.codec.is_some()
        && !token.zip(stream.span).is_some_and(|(token, span)| {
            hash.codec.is_some()
                && token
                    .state_binding
                    .as_ref()
                    .is_some_and(|binding| stream_dependency(source, span, binding.identifier))
                && stream_initial_state(source, span, hash.span.unwrap(), token, wiring).is_ok()
        })
    {
        stream.codec = None;
        stream.reason = Some(
            token
                .zip(stream.span)
                .and_then(|(token, span)| {
                    hash.span.and_then(|hash| {
                        stream_initial_state(source, span, hash, token, wiring).err()
                    })
                })
                .unwrap_or("stream does not capture the certified token state")
                .into(),
        );
    }
    // A helper that calls a captured character accessor needs that dependency
    // certified as well; the source-only resolver follows single-origin aliases.
    if hash.codec.is_some()
        && !character_dependency(source, encoding.seed_link.hash.as_ref().unwrap().span, None)
    {
        hash.codec = None;
        hash.reason = Some("UTF-16 character accessor dependency is not certified".into());
    }
    if hash.codec.is_some()
        && !unshadowed(
            source,
            hash.span.unwrap(),
            &["ArrayBuffer", "Int8Array", "Int32Array", "TypeError"],
        )
    {
        hash.codec = None;
        hash.reason = Some("hash builtin dependency is shadowed".into());
    }
    if hash.codec.is_none() {
        stream.codec = None;
        stream.reason = Some("stream hash dependency is not certified".into());
    }
    let string_candidate = encoding
        .string
        .as_ref()
        .and_then(|s| candidate(s.factory.span));
    let mut string = proof(
        string_candidate,
        include_str!("reference/string.js"),
        "modified-utf8-intern-129/1",
        false,
    );
    if string.codec.is_none() {
        string = proof(
            string_candidate,
            include_str!("reference/string-main.js"),
            "modified-utf8-intern-129/1",
            false,
        );
    }
    if string.codec.is_none() {
        // Same complete factory with a direct second NUL write and the
        // final surrogate byte masked in its temporary before the store.
        string = proof(
            string_candidate,
            include_str!("reference/string-direct-nul-masked-tail.js"),
            "modified-utf8-intern-129/1",
            false,
        );
    }
    if string.codec.is_some()
        && (integer.codec.is_none()
            || !character_dependency(
                source,
                string.span.unwrap(),
                encoding.number.as_ref().map(|n| n.function.span),
            ))
    {
        string.codec = None;
        string.reason = Some("string character/number dependencies are not certified".into());
    }
    if string.codec.is_some() && !unshadowed(source, string.span.unwrap(), &["Number", "Object"]) {
        string.codec = None;
        string.reason = Some("string factory builtin dependency is shadowed".into());
    }
    AlgorithmReport {
        hash,
        stream,
        integer,
        float_prefix,
        string,
        float: unsupported("float path and byte order are not certified"),
    }
}

pub(super) fn unshadowed(source: &str, span: SourceSpan, names: &[&str]) -> bool {
    let arena = Allocator::default();
    let parsed = Parser::new(&arena, source, SourceType::mjs()).parse();
    if !parsed.errors.is_empty() {
        return false;
    }
    let built = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    if !built.errors.is_empty() {
        return false;
    }
    let semantic = built.semantic;
    let mut scan = Scan {
        ids: Ids::default(),
        scopes: semantic.scoping(),
    };
    scan.visit_program(&parsed.program);
    scan.ids
        .references
        .iter()
        .filter(|(s, name, _, _)| {
            span.start <= s.start && s.end <= span.end && names.contains(&name.as_str())
        })
        .all(|(_, _, symbol, _)| symbol.is_none())
}

fn stream_dependency(source: &str, factory: SourceSpan, state: SourceSpan) -> bool {
    let arena = Allocator::default();
    let parsed = Parser::new(&arena, source, SourceType::mjs()).parse();
    if !parsed.errors.is_empty() {
        return false;
    }
    let built = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    if !built.errors.is_empty() {
        return false;
    }
    let semantic = built.semantic;
    let mut scan = Scan {
        ids: Ids::default(),
        scopes: semantic.scoping(),
    };
    scan.visit_program(&parsed.program);
    let mut captured = false;
    scan.ids
        .references
        .iter()
        .filter(|(s, _, _, _)| factory.start <= s.start && s.end <= factory.end)
        .all(|(_, _, symbol, _)| {
            let Some(symbol) = symbol else {
                return false;
            };
            let span = semantic
                .scoping()
                .symbol_span(oxc_semantic::SymbolId::from_usize(*symbol as usize));
            if factory.start <= span.start && span.end <= factory.end {
                true
            } else {
                captured = true;
                span.start == state.start && span.end == state.end
            }
        })
        && captured
}

// A function dependency has one immutable origin. In particular a label or
// a previous assignment to a hash-shaped helper is not a value proof.
fn function_origin<'a>(
    expression: &Expression<'a>,
    scopes: &Scoping,
    nodes: &oxc_semantic::AstNodes<'a>,
    depth: usize,
) -> Option<SourceSpan> {
    if depth > 16 {
        return None;
    }
    match expression.without_parentheses() {
        Expression::FunctionExpression(f) => Some(f.span.into()),
        Expression::Identifier(id) => {
            let symbol = id
                .reference_id
                .get()
                .and_then(|r| scopes.get_reference(r).symbol_id())?;
            if !scopes.symbol_redeclarations(symbol).is_empty() {
                return None;
            }
            let declaration = scopes.symbol_declaration(symbol);
            let writes: Vec<_> = scopes
                .get_resolved_references(symbol)
                .filter(|r| r.is_write())
                .collect();
            match nodes.kind(declaration) {
                AstKind::Function(f) if writes.is_empty() => Some(f.span.into()),
                AstKind::VariableDeclarator(d) if writes.is_empty() => {
                    function_origin(d.init.as_ref()?, scopes, nodes, depth + 1)
                }
                AstKind::VariableDeclarator(d) if d.init.is_none() && writes.len() == 1 => {
                    let definition = crate::signals::extract::reaching::expression_value_at_use(
                        scopes, nodes, expression,
                    )
                    .ok()?;
                    function_origin(definition.value, scopes, nodes, depth + 1)
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// Resolve the value of a reused local at this call, rather than requiring
/// every assignment to its storage slot to hold a function. The reaching
/// definition checker rejects intervening writes, callbacks and loop edges.
fn callsite_function_origin<'a>(
    expression: &Expression<'a>,
    scopes: &Scoping,
    nodes: &oxc_semantic::AstNodes<'a>,
    depth: usize,
) -> Option<SourceSpan> {
    if depth > 16 {
        return None;
    }
    if let Ok(definition) =
        crate::signals::extract::reaching::expression_value_at_use(scopes, nodes, expression)
    {
        return callsite_function_origin(definition.value, scopes, nodes, depth + 1);
    }
    match expression.without_parentheses() {
        Expression::FunctionExpression(f) => Some(f.span.into()),
        Expression::Identifier(id) => {
            let symbol = id
                .reference_id
                .get()
                .and_then(|r| scopes.get_reference(r).symbol_id())?;
            // Function declarations are hoisted. Variable initializers are
            // not: a failed reaching proof must not admit a later/conditional
            // initialization merely because the binding has no other writes.
            match nodes.kind(scopes.symbol_declaration(symbol)) {
                AstKind::Function(f)
                    if scopes.symbol_redeclarations(symbol).is_empty()
                        && scopes
                            .get_resolved_references(symbol)
                            .all(|r| !r.is_write()) =>
                {
                    Some(f.span.into())
                }
                AstKind::VariableDeclarator(d)
                    if enclosing_activation(nodes, scopes.symbol_declaration(symbol))
                        != enclosing_activation(
                            nodes,
                            scopes.get_reference(id.reference_id.get()?).node_id(),
                        )
                        && scopes.symbol_redeclarations(symbol).is_empty() =>
                {
                    let writes: Vec<_> = scopes
                        .get_resolved_references(symbol)
                        .filter(|r| r.is_write())
                        .collect();
                    let (value, definition) = match (d.init.as_ref(), writes.as_slice()) {
                        (Some(value), []) => (value, scopes.symbol_declaration(symbol)),
                        (None, [reference]) => {
                            let definition = nodes.parent_id(reference.node_id());
                            let AstKind::AssignmentExpression(a) = nodes.kind(definition) else {
                                return None;
                            };
                            if a.operator != oxc_syntax::operator::AssignmentOperator::Assign {
                                return None;
                            }
                            (&a.right, definition)
                        }
                        _ => return None,
                    };
                    // An outer helper slot may be assigned once before the
                    // collector is invoked. Only a whole, unconditional
                    // assignment/declaration is admitted, never a branch or
                    // loop and never a subsequently overwritten helper value.
                    let statement = nodes.parent_id(definition);
                    if !matches!(
                        nodes.kind(statement),
                        AstKind::ExpressionStatement(_) | AstKind::VariableDeclaration(_)
                    ) || !matches!(
                        nodes.kind(nodes.parent_id(statement)),
                        AstKind::FunctionBody(_) | AstKind::Program(_)
                    ) {
                        return None;
                    }
                    callsite_function_origin(value, scopes, nodes, depth + 1)
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// Recover a NUMBER factory omitted by extraction when a callable local is
/// subsequently reused as its returned cursor. This certifies the complete
/// integer body and its builtin dependency, not the recognizer's role label.
pub(super) fn number_at_call(
    source: &str,
    write: &crate::signals::extract::contracts::WriteNode,
) -> AlgorithmProof {
    let refused = || unsupported("concrete number call origin is not certified");
    let arena = Allocator::default();
    let parsed = Parser::new(&arena, source, SourceType::mjs()).parse();
    if !parsed.errors.is_empty() {
        return refused();
    }
    let built = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    if !built.errors.is_empty() {
        return refused();
    }
    let semantic = built.semantic;
    if semantic.nodes().iter().any(|n| {
        matches!(n.kind(), AstKind::WithStatement(_))
            || matches!(n.kind(), AstKind::CallExpression(c)
                if matches!(c.callee.without_parentheses(), Expression::Identifier(id) if id.name == "eval"))
    }) {
        return refused();
    }
    let Some(call) = semantic.nodes().iter().find_map(|n| match n.kind() {
        AstKind::CallExpression(c) if c.span == write.span => Some(c),
        _ => None,
    }) else {
        return refused();
    };
    if call.optional
        || call.arguments.len() != 4
        || call.arguments.iter().any(|a| a.as_expression().is_none())
        || call.arguments[1].span() != write.value_span
    {
        return refused();
    }
    let Some(span) =
        callsite_function_origin(&call.callee, semantic.scoping(), semantic.nodes(), 0)
    else {
        return refused();
    };
    let mut result = proof(
        source
            .get(span.start as usize..span.end as usize)
            .map(|s| (span, s)),
        include_str!("reference/number.js"),
        "sign-magnitude-5-7/1",
        true,
    );
    if result.codec.is_some() && !unshadowed(source, span, &["Number"]) {
        result.codec = None;
        result.reason = Some("number conversion dependency is shadowed".into());
    }
    result
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StateDefinition {
    initializer: SourceSpan,
    assignment: SourceSpan,
    state: SourceSpan,
}
fn enclosing_activation(
    nodes: &oxc_semantic::AstNodes<'_>,
    mut id: oxc_semantic::NodeId,
) -> Option<oxc_semantic::NodeId> {
    loop {
        let p = nodes.parent_id(id);
        if p == id {
            return None;
        }
        id = p;
        if matches!(
            nodes.kind(id),
            AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) | AstKind::Program(_)
        ) {
            return Some(id);
        }
    }
}
fn stream_state_definition(
    source: &str,
    factory: SourceSpan,
    hash: SourceSpan,
    token: &crate::signals::extract::wiring::TokenProvenanceWiring,
    wiring: &WiringProperties,
) -> Result<StateDefinition, &'static str> {
    if !stream_dependency(
        source,
        factory,
        token
            .state_binding
            .as_ref()
            .ok_or("missing state binding")?
            .identifier,
    ) {
        return Err("stream state capture differs");
    }
    let arena = Allocator::default();
    let parsed = Parser::new(&arena, source, SourceType::mjs()).parse();
    if !parsed.errors.is_empty() {
        return Err("source parse diagnostics");
    }
    let built = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    if !built.errors.is_empty() {
        return Err("source semantic diagnostics");
    }
    let semantic = built.semantic;
    let scopes = semantic.scoping();
    let nodes = semantic.nodes();
    // Direct eval/with can change bindings without a statically visible write.
    if nodes.iter().any(|n| matches!(n.kind(), AstKind::WithStatement(_))
        || matches!(n.kind(), AstKind::CallExpression(c) if
            matches!(c.callee.without_parentheses(), Expression::Identifier(id) if id.name == "eval"))) { return Err("dynamic scope"); }
    let Some(state) = token.state_binding.as_ref() else {
        return Err("missing state binding");
    };
    if token.hash_function.as_ref().map(|f| f.span) != Some(hash) {
        return Err("hash metadata differs");
    }
    let Some(symbol) = scopes.symbol_ids().find(|s| {
        let span = scopes.symbol_span(*s);
        span.start == state.identifier.start && span.end == state.identifier.end
    }) else {
        return Err("state symbol missing");
    };
    if !scopes.symbol_redeclarations(symbol).is_empty() {
        return Err("state redeclaration");
    }
    let declaration = scopes.symbol_declaration(symbol);
    let AstKind::VariableDeclarator(d) = nodes.kind(declaration) else {
        return Err("state is not a local");
    };
    let writes: Vec<_> = scopes
        .get_resolved_references(symbol)
        .filter(|r| r.is_write())
        .collect();
    let (value, definition) = if let Some(value) = &d.init {
        if !writes.is_empty() {
            return Err("initialized state has further writes");
        }
        (value, declaration)
    } else {
        if writes.len() != 1 {
            return Err("state needs exactly one write");
        }
        let parent = nodes.parent_id(writes[0].node_id());
        let AstKind::AssignmentExpression(a) = nodes.kind(parent) else {
            return Err("state write is not an assignment");
        };
        if a.operator != AssignmentOperator::Assign {
            return Err("state write is not plain");
        }
        (&a.right, parent)
    };
    let Expression::CallExpression(call) = value.without_parentheses() else {
        return Err("state value is not a call");
    };
    if call.arguments.len() != 1 || function_origin(&call.callee, scopes, nodes, 0) != Some(hash) {
        return Err("state hash function origin differs");
    }
    let Some(argument) = call.arguments[0].as_expression() else {
        return Err("state hash argument is not an expression");
    };
    let input = argument.span();
    if source.get(input.start as usize..input.end as usize) != Some(token.input_source.as_str()) {
        return Err("state hash input differs");
    }
    // Require a plain unconditional definition in the function/program body.
    let statement = nodes.parent_id(definition);
    let statement = if matches!(nodes.kind(statement), AstKind::VariableDeclaration(_)) {
        statement
    } else if matches!(nodes.kind(statement), AstKind::ExpressionStatement(_)) {
        statement
    } else {
        return Err("state definition is not a whole statement");
    };
    let list = nodes.parent_id(statement);
    if !matches!(
        nodes.kind(list),
        AstKind::FunctionBody(_) | AstKind::Program(_)
    ) {
        return Err("state definition is conditional");
    }
    let Some(root) = enclosing_activation(nodes, definition) else {
        return Err("state definition has no owner");
    };
    let state_definition = StateDefinition {
        initializer: nodes.get_node(root).span().into(),
        assignment: nodes.get_node(statement).span().into(),
        state: state.identifier,
    };
    lifecycle::verify_token_value(
        argument,
        call.span.into(),
        &state_definition,
        scopes,
        nodes,
        wiring,
    )
    .map_err(|e| e.0)?;
    Ok(state_definition)
}

fn stream_initial_state(
    source: &str,
    factory: SourceSpan,
    hash: SourceSpan,
    token: &crate::signals::extract::wiring::TokenProvenanceWiring,
    wiring: &WiringProperties,
) -> Result<(), &'static str> {
    fn owner(
        nodes: &oxc_semantic::AstNodes<'_>,
        mut node: oxc_semantic::NodeId,
    ) -> Option<oxc_semantic::NodeId> {
        loop {
            let parent = nodes.parent_id(node);
            if parent == node {
                return None;
            }
            node = parent;
            if matches!(
                nodes.kind(node),
                AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) | AstKind::Program(_)
            ) {
                return Some(node);
            }
        }
    }
    // A closure can run before its textual body. Follow every reference to a
    // nested function back to its enclosing initialization statement list;
    // creation of a function expression itself is conservatively an escape.
    fn after_initialization(
        nodes: &oxc_semantic::AstNodes<'_>,
        scopes: &Scoping,
        node: oxc_semantic::NodeId,
        root: oxc_semantic::NodeId,
        initialized: u32,
        visiting: &mut Vec<oxc_semantic::NodeId>,
    ) -> bool {
        let Some(enclosing) = owner(nodes, node) else {
            return false;
        };
        if enclosing == root {
            return nodes.get_node(node).span().start >= initialized;
        }
        if visiting.contains(&enclosing) {
            return false;
        }
        let AstKind::Function(function) = nodes.kind(enclosing) else {
            return false;
        };
        visiting.push(enclosing);
        let ok = if function.r#type == FunctionType::FunctionDeclaration {
            function
                .id
                .as_ref()
                .and_then(|id| id.symbol_id.get())
                .is_some_and(|symbol| {
                    let refs: Vec<_> = scopes.get_resolved_references(symbol).collect();
                    !refs.is_empty()
                        && refs.iter().all(|r| {
                            !r.is_write()
                                && after_initialization(
                                    nodes,
                                    scopes,
                                    r.node_id(),
                                    root,
                                    initialized,
                                    visiting,
                                )
                        })
                })
        } else {
            after_initialization(nodes, scopes, enclosing, root, initialized, visiting)
        };
        visiting.pop();
        ok
    }
    let definition = stream_state_definition(source, factory, hash, token, wiring)?;
    let arena = Allocator::default();
    let parsed = Parser::new(&arena, source, SourceType::mjs()).parse();
    let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
    let scopes = semantic.scoping();
    let nodes = semantic.nodes();
    let root = nodes
        .iter()
        .find(|n| {
            matches!(
                n.kind(),
                AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) | AstKind::Program(_)
            ) && SourceSpan::from(n.span()) == definition.initializer
                && !matches!(n.kind(), AstKind::Program(_))
        })
        .ok_or("initializer node missing")?
        .id();
    let initialized = definition.assignment.end;
    let Some(factory_node) = nodes.iter().find(|n|
        matches!(n.kind(), AstKind::Function(f) if f.span.start == factory.start && f.span.end == factory.end)) else { return Err("stream function missing"); };
    let AstKind::Function(f) = factory_node.kind() else {
        return Err("stream node is not a function");
    };
    let Some(factory_symbol) = f.id.as_ref().and_then(|id| id.symbol_id.get()) else {
        return Err("stream function binding missing");
    };
    // Taking a factory's value before token initialization does not read the
    // token state. Follow its single-origin aliases to actual invocation sites.
    let mut pending = vec![factory_symbol];
    let mut seen = Vec::new();
    let mut calls = 0;
    while let Some(symbol) = pending.pop() {
        if seen.contains(&symbol) {
            continue;
        }
        seen.push(symbol);
        if !scopes.symbol_redeclarations(symbol).is_empty() {
            return Err("stream factory alias is redeclared");
        }
        for reference in scopes.get_resolved_references(symbol) {
            if reference.is_write() {
                if symbol == factory_symbol {
                    return Err("stream factory is overwritten");
                }
                continue;
            }
            let id = reference.node_id();
            let span = nodes.get_node(id).span();
            match nodes.kind(nodes.parent_id(id)) {
                AstKind::VariableDeclarator(d)
                    if d.init.as_ref().is_some_and(|e| e.span() == span) =>
                {
                    let BindingPattern::BindingIdentifier(binding) = &d.id else {
                        return Err("stream alias is destructured");
                    };
                    pending.push(
                        binding
                            .symbol_id
                            .get()
                            .ok_or("stream alias binding missing")?,
                    );
                }
                AstKind::AssignmentExpression(a)
                    if a.operator == AssignmentOperator::Assign && a.right.span() == span =>
                {
                    let AssignmentTarget::AssignmentTargetIdentifier(binding) = &a.left else {
                        return Err("stream alias escapes through a property");
                    };
                    pending.push(
                        binding
                            .reference_id
                            .get()
                            .and_then(|r| scopes.get_reference(r).symbol_id())
                            .ok_or("stream alias is not local")?,
                    );
                }
                AstKind::CallExpression(call) if call.callee.span() == span => {
                    if function_origin(&call.callee, scopes, nodes, 0) != Some(factory) {
                        return Err("stream invocation has an unproven alias origin");
                    }
                    if !after_initialization(nodes, scopes, id, root, initialized, &mut vec![]) {
                        return Err("stream invocation can precede state initialization");
                    }
                    calls += 1;
                }
                _ => return Err("stream factory escapes through an unsupported use"),
            }
        }
    }
    if calls == 0 {
        Err("stream factory has no proven invocation")
    } else {
        Ok(())
    }
}

// Resolve the hash's character accessor to a single-origin plain wrapper.
fn character_dependency(source: &str, hash_span: SourceSpan, number: Option<SourceSpan>) -> bool {
    let arena = Allocator::default();
    let parsed = Parser::new(&arena, source, SourceType::mjs()).parse();
    if !parsed.errors.is_empty() {
        return false;
    }
    let built = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    if !built.errors.is_empty() {
        return false;
    }
    let semantic = built.semantic;
    accessor_dependency(
        source,
        &parsed.program,
        semantic.scoping(),
        semantic.nodes(),
        hash_span,
        number,
    )
}

fn accessor_dependency<'a>(
    source: &str,
    program: &Program<'a>,
    scopes: &Scoping,
    nodes: &oxc_semantic::AstNodes<'a>,
    hash_span: SourceSpan,
    number: Option<SourceSpan>,
) -> bool {
    #[derive(Clone)]
    enum Def {
        Alias(u32),
        Function(SourceSpan),
        Accessor,
        Other,
    }
    struct Scan<'s, 'a> {
        scopes: &'s Scoping,
        nodes: &'s oxc_semantic::AstNodes<'a>,
        defs: BTreeMap<u32, Vec<Def>>,
        calls: Vec<(SourceSpan, Option<u32>, usize)>,
    }
    impl Scan<'_, '_> {
        fn symbol(&self, id: &IdentifierReference<'_>) -> Option<u32> {
            id.reference_id
                .get()
                .and_then(|r| self.scopes.get_reference(r).symbol_id())
                .map(|s| s.index() as u32)
        }
        fn def(&self, e: &Expression<'_>) -> Def {
            self.def_depth(e, 0)
        }
        fn def_depth(&self, e: &Expression<'_>, depth: usize) -> Def {
            if depth > 16 {
                return Def::Other;
            }
            match e.without_parentheses() {
                Expression::Identifier(id) => {
                    if let Ok(value) = crate::signals::extract::reaching::expression_value_at_use(
                        self.scopes,
                        self.nodes,
                        e,
                    ) {
                        return self.def_depth(value.value, depth + 1);
                    }
                    self.symbol(id).map(Def::Alias).unwrap_or(Def::Other)
                }
                Expression::FunctionExpression(f) => Def::Function(f.span.into()),
                other if bound_accessor(other, self.scopes) => Def::Accessor,
                _ => Def::Other,
            }
        }
    }
    impl<'a> Visit<'a> for Scan<'_, 'a> {
        fn visit_function(&mut self, f: &Function<'a>, flags: oxc_syntax::scope::ScopeFlags) {
            if let Some(s) = f.id.as_ref().and_then(|i| i.symbol_id.get()) {
                self.defs
                    .entry(s.index() as u32)
                    .or_default()
                    .push(Def::Function(f.span.into()));
            }
            walk::walk_function(self, f, flags);
        }
        fn visit_variable_declarator(&mut self, d: &VariableDeclarator<'a>) {
            if let (BindingPattern::BindingIdentifier(b), Some(e)) = (&d.id, &d.init) {
                if let Some(s) = b.symbol_id.get() {
                    let v = self.def(e);
                    self.defs.entry(s.index() as u32).or_default().push(v);
                }
            }
            walk::walk_variable_declarator(self, d);
        }
        fn visit_assignment_expression(&mut self, e: &AssignmentExpression<'a>) {
            if let AssignmentTarget::AssignmentTargetIdentifier(id) = &e.left {
                if let Some(s) = self.symbol(id) {
                    let v = if e.operator == oxc_syntax::operator::AssignmentOperator::Assign {
                        self.def(&e.right)
                    } else {
                        Def::Other
                    };
                    self.defs.entry(s).or_default().push(v);
                }
            }
            walk::walk_assignment_expression(self, e);
        }
        fn visit_update_expression(&mut self, e: &UpdateExpression<'a>) {
            if let SimpleAssignmentTarget::AssignmentTargetIdentifier(id) = &e.argument {
                if let Some(s) = self.symbol(id) {
                    self.defs.entry(s).or_default().push(Def::Other);
                }
            }
            walk::walk_update_expression(self, e);
        }
        fn visit_call_expression(&mut self, e: &CallExpression<'a>) {
            if let Expression::Identifier(id) = &e.callee {
                // Builtin conversion calls are verified separately. Every
                // other external call must resolve to its concrete role.
                if id.name != "Number" {
                    self.calls
                        .push((e.span.into(), self.symbol(id), e.arguments.len()));
                }
            }
            walk::walk_call_expression(self, e);
        }
    }
    let mut scan = Scan {
        scopes,
        nodes,
        defs: BTreeMap::new(),
        calls: vec![],
    };
    scan.visit_program(program);
    let calls: Vec<_> = scan
        .calls
        .iter()
        .filter(|(s, symbol, _)| {
            let local = symbol.is_some_and(|symbol| {
                let declared =
                    scopes.symbol_span(oxc_semantic::SymbolId::from_usize(symbol as usize));
                hash_span.start <= declared.start && declared.end <= hash_span.end
            });
            hash_span.start <= s.start && s.end <= hash_span.end && !local
        })
        .collect();
    if calls.is_empty() {
        return false;
    }
    calls.into_iter().all(|(_, initial, arity)| {
        // Whole-body equivalence fixes the argument expressions and positions:
        // four arguments are (stream, intern index, buffer, cursor); character
        // access is (one UTF-16 unit[, zero]). These roles cannot substitute.
        let number_role = number.is_some() && *arity == 4;
        if !number_role && !matches!(*arity, 1 | 2) {
            return false;
        }
        let Some(mut symbol) = *initial else {
            return false;
        };
        for _ in 0..16 {
            match scan.defs.get(&symbol).map(Vec::as_slice) {
                Some([Def::Alias(next)]) => symbol = *next,
                Some([Def::Function(span)]) => {
                    if number_role {
                        return Some(*span) == number;
                    }
                    if Some(*span) == number {
                        return false;
                    }
                    let Some(text) = source.get(span.start as usize..span.end as usize) else {
                        return false;
                    };
                    let text = if text.starts_with("function(") || text.starts_with("function (") {
                        text.replacen("function", "function accessor", 1)
                    } else {
                        text.to_string()
                    };
                    return canonical(&text, false)
                        .zip(canonical("function unit(s){return s.charCodeAt();}", false))
                        .is_some_and(|(a, b)| a == b);
                }
                Some([Def::Accessor]) => return !number_role,
                _ => return false,
            }
        }
        false
    })
}
fn bound_accessor(e: &Expression<'_>, scopes: &Scoping) -> bool {
    fn member<'e, 'a>(e: &'e Expression<'a>, name: &str) -> Option<&'e Expression<'a>> {
        if let Expression::StaticMemberExpression(m) = e.without_parentheses() {
            if m.property.name == name {
                return Some(&m.object);
            }
        }
        None
    }
    fn global(e: &Expression<'_>, name: &str, scopes: &Scoping) -> bool {
        matches!(e.without_parentheses(),Expression::Identifier(id) if id.name==name && id.reference_id.get().is_some_and(|r|scopes.get_reference(r).symbol_id().is_none()))
    }
    let Expression::CallExpression(call) = e.without_parentheses() else {
        return false;
    };
    let Some(object) = member(&call.callee, "bind").and_then(|e| member(e, "call")) else {
        return false;
    };
    if !global(object, "Array", scopes) || call.arguments.len() != 1 {
        return false;
    }
    call.arguments[0]
        .as_expression()
        .and_then(|e| member(e, "charCodeAt"))
        .and_then(|e| member(e, "prototype"))
        .is_some_and(|e| global(e, "String", scopes))
}

#[cfg(test)]
mod checkpoint_tests {
    use super::*;
    #[test]
    fn whole_body_proofs_reject_changes_to_recurrence_warmup_and_hash() {
        let span = SourceSpan::new(0, 1);
        for reference in [
            include_str!("reference/stream.js"),
            include_str!("reference/hash.js"),
        ] {
            assert!(proof(Some((span, reference)), reference, "test", false)
                .codec
                .is_some());
        }
        let stream = include_str!("reference/stream.js");
        for changed in [
            stream.replacen("26", "25", 1),
            stream.replacen("local6();", "", 1),
            stream.replacen("& 255", "& 254", 1),
        ] {
            assert!(proof(Some((span, &changed)), stream, "test", false)
                .codec
                .is_none());
        }
        let hash = include_str!("reference/hash.js");
        let changed = hash.replacen("<< 5", "<< 6", 1);
        assert!(proof(Some((span, &changed)), hash, "test", false)
            .codec
            .is_none());
    }
    #[test]
    fn integer_certification_is_separate_from_float_compatibility() {
        let number = include_str!("reference/number.js");
        let span = SourceSpan::new(0, 1);
        assert!(proof(Some((span, number)), number, "test", true)
            .codec
            .is_some());
        let changed = number.replacen("< 4096", "< 4095", 1);
        assert!(proof(Some((span, &changed)), number, "test", true)
            .codec
            .is_none());
        let float_changed = number.replacen("Math.log", "Math.sqrt", 1);
        assert!(proof(Some((span, &float_changed)), number, "test", true)
            .codec
            .is_some());
        assert!(proof(Some((span, &float_changed)), number, "test", false)
            .codec
            .is_none());
        assert!(float_prefix_proof(Some((span, &float_changed)), number)
            .codec
            .is_some());
        let marker_changed = number.replacen("= local1() ^ 128;", "= local1() ^ 129;", 1);
        assert_ne!(marker_changed, number);
        assert!(proof(Some((span, &marker_changed)), number, "test", true)
            .codec
            .is_some());
        assert!(float_prefix_proof(Some((span, &marker_changed)), number)
            .codec
            .is_none());
    }
    #[test]
    fn alpha_equivalence_keeps_binding_and_parameter_identity() {
        assert_eq!(
            canonical("function f(a){let b;b=a;return b;}", false),
            canonical("function g(x){let y=x;return y;}", false)
        );
        assert_ne!(
            canonical("function f(a,b){return a-b;}", false),
            canonical("function f(a,b){return b-a;}", false)
        );
        assert_ne!(
            canonical("function f(){let count=0;return ()=>++count;}", false),
            canonical("function f(){const count=0;return ()=>++count;}", false)
        );
        assert_ne!(
            canonical("function f(a){return external(a);}", false),
            canonical("function f(a){let external=a;return external(a);}", false)
        );
    }
}
#[cfg(test)]
#[path = "algorithms_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "algorithms_lifecycle_tests.rs"]
mod lifecycle_tests;

#[path = "algorithms_lifecycle.rs"]
mod lifecycle;
pub use lifecycle::{
    InitializationContext, InitializationEpoch, StreamReadiness, StreamReadinessError,
};
