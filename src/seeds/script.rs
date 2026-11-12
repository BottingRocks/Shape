//! Local classification of a saved JavaScript file. No network access.
//!
//! A file is an **async launcher** when it provably loads another script: some
//! expression that resolves to `createElement("script")` has its `src` set to
//! a statically known string. That string is the kernel URL. The proof goes
//! through Oxc's semantic model (bindings and their writes), never through
//! statement positions or "the first string that looks like a URL".
//!
//! A file is a **raw kernel** when it is valid JavaScript, loads no script, and
//! has the gross shape of a kernel (a function-bearing program of non-trivial
//! size). Everything else — HTML, error pages, stray snippets — is an error.

use std::collections::HashMap;

use oxc_allocator::Allocator;
use oxc_ast::ast::{Argument, AssignmentTarget, Expression};
use oxc_ast::AstKind;
use oxc_parser::Parser;
use oxc_semantic::{AstNodes, NodeId, Scoping, SemanticBuilder, SymbolId};
use oxc_span::{GetSpan, SourceType};
use oxc_syntax::operator::{AssignmentOperator, BinaryOperator, UnaryOperator};
use oxc_syntax::scope::ScopeFlags;
use sha2::{Digest, Sha256};

use crate::seeds::error::{Error, Result};
use crate::seeds::seed::{self, DEFAULT_SEED_PARAM};

/// Smallest file accepted as a raw kernel. Real kernels are hundreds of
/// kilobytes; this only keeps stubs and error snippets from passing as one.
pub const MIN_KERNEL_BYTES: usize = 4096;

/// Bound on binding-to-binding hops when resolving a static value.
const MAX_RESOLVE_DEPTH: u8 = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Script {
    Launcher(Launcher),
    Kernel(KernelSummary),
}

impl Script {
    pub fn describe(&self) -> &'static str {
        match self {
            Script::Launcher(_) => "an async launcher",
            Script::Kernel(_) => "a raw kernel",
        }
    }
}

/// What a launcher statically tells us: the URL reference it loads the kernel
/// from, exactly as written (it may be relative).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Launcher {
    pub kernel_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelSummary {
    pub bytes: usize,
    pub sha256: String,
    pub statements: usize,
    pub functions: usize,
}

/// Lowercase hex SHA-256 of `bytes`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Classify `bytes` as a launcher or a raw kernel.
pub fn classify(bytes: &[u8]) -> Result<Script> {
    classify_with_seed_param(bytes, DEFAULT_SEED_PARAM)
}

/// Classify `bytes`, selecting a proven script load whose URL contains the
/// configured seed query parameter when a launcher has static helper loads.
pub fn classify_with_seed_param(bytes: &[u8], seed_param: &str) -> Result<Script> {
    let source = std::str::from_utf8(bytes).map_err(|_| Error::NotUtf8)?;
    let body = source.trim_start_matches('\u{feff}').trim_start();
    if body.is_empty() {
        return Err(Error::Empty);
    }
    // Checked before parsing so a block page gets a precise error; Oxc would
    // reject it as a syntax error anyway.
    if body.starts_with('<') {
        return Err(Error::Html);
    }

    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::script()).parse();
    if let Some(error) = parsed.errors.first() {
        return Err(Error::NotJavaScript {
            message: error.to_string(),
        });
    }
    if parsed.panicked {
        return Err(Error::NotJavaScript {
            message: "parser could not recover".to_owned(),
        });
    }
    let program = parsed.program;
    let semantic = SemanticBuilder::new().build(&program).semantic;
    let scoping = semantic.scoping();

    let has_unproven_scope = scoping.scope_descendants_from_root().any(|scope_id| {
        scoping
            .scope_flags(scope_id)
            .intersects(ScopeFlags::DirectEval | ScopeFlags::With)
    });

    // One pass over the node table: where each binding gets its value, and
    // every write/removal affecting a `src` property.
    let mut definitions: HashMap<SymbolId, Vec<Definition<'_>>> = HashMap::new();
    let mut sinks: Vec<(&Expression<'_>, &Expression<'_>, NodeId)> = Vec::new();
    let mut writes: Vec<WriteSite<'_>> = Vec::new();
    let mut write_mutations: Vec<&Expression<'_>> = Vec::new();
    let mut removals: Vec<SrcRemoval<'_>> = Vec::new();
    let mut functions = 0usize;
    for node in semantic.nodes().iter() {
        match node.kind() {
            AstKind::VariableDeclarator(declarator) => {
                if let (Some(binding), Some(init)) =
                    (declarator.id.get_binding_identifier(), &declarator.init)
                {
                    if let Some(symbol) = binding.symbol_id.get() {
                        definitions
                            .entry(symbol)
                            .or_default()
                            .push(Definition::Value {
                                value: init,
                                node_id: declarator.node_id(),
                            });
                    }
                }
            }
            AstKind::AssignmentExpression(assignment) => {
                let plain = assignment.operator == AssignmentOperator::Assign;
                if let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left {
                    let symbol = target
                        .reference_id
                        .get()
                        .and_then(|reference| scoping.get_reference(reference).symbol_id());
                    if let Some(symbol) = symbol {
                        definitions.entry(symbol).or_default().push(if plain {
                            Definition::Assigned {
                                value: &assignment.right,
                                node_id: assignment.node_id(),
                            }
                        } else {
                            Definition::Compound {
                                operator: assignment.operator,
                                right: &assignment.right,
                                node_id: assignment.node_id(),
                            }
                        });
                    }
                } else if let Some(member) = assignment.left.as_member_expression() {
                    if plain && member.static_property_name() == Some("src") {
                        sinks.push((member.object(), &assignment.right, member_node_id(member)));
                    }
                    if member.static_property_name() == Some("write") {
                        write_mutations.push(member.object());
                    }
                }
            }
            AstKind::UnaryExpression(unary) if unary.operator == UnaryOperator::Delete => {
                if let Some(member) = unary.argument.as_member_expression() {
                    if member.static_property_name() == Some("src") {
                        removals.push(SrcRemoval {
                            element: member.object(),
                            attribute: None,
                        });
                    }
                }
            }
            AstKind::CallExpression(call) => {
                // element.setAttribute("src", value), or a removal that may
                // invalidate a previously proven src.
                let Some(member) = call.callee.get_inner_expression().as_member_expression() else {
                    continue;
                };
                match member.static_property_name() {
                    Some("setAttribute") => {
                        if let [Argument::StringLiteral(name), value] = call.arguments.as_slice() {
                            if let (true, Some(value)) = (
                                name.value.eq_ignore_ascii_case("src"),
                                value.as_expression(),
                            ) {
                                sinks.push((member.object(), value, member_node_id(member)));
                            }
                        }
                    }
                    Some("removeAttribute") => {
                        removals.push(SrcRemoval {
                            element: member.object(),
                            attribute: call.arguments.first().and_then(Argument::as_expression),
                        });
                    }
                    Some("write") => {
                        if call.arguments.len() == 1 {
                            if let Some(value) =
                                call.arguments.first().and_then(Argument::as_expression)
                            {
                                writes.push(WriteSite {
                                    object: member.object(),
                                    value,
                                    node_id: call.node_id(),
                                });
                            } else {
                                writes.push(WriteSite {
                                    object: member.object(),
                                    value: &call.callee,
                                    node_id: call.node_id(),
                                });
                            }
                        } else {
                            writes.push(WriteSite {
                                object: member.object(),
                                value: &call.callee,
                                node_id: call.node_id(),
                            });
                        }
                    }
                    _ => {}
                }
            }
            AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) => functions += 1,
            _ => {}
        }
    }

    let resolver = Resolver {
        scoping,
        definitions: &definitions,
        nodes: semantic.nodes(),
    };
    let mut candidates: Vec<String> = Vec::new();
    let mut dynamic = false;
    let mut saw_script_element = false;
    for &(element, value, sink_node_id) in &sinks {
        let sink_ok = resolver.sink_block(sink_node_id).is_some();
        let element_ok = resolver.is_script_element_at_sink(element, 0, sink_node_id);
        if !sink_ok || !element_ok {
            if resolver.is_script_element(element, 0) {
                saw_script_element = true;
                dynamic = true;
            }
            continue;
        }
        saw_script_element = true;
        if removals.iter().any(|removal| {
            resolver.same_element(element, removal.element, 0) && resolver.removes_src(removal)
        }) {
            return Err(Error::UnprovenLauncher {
                reason: "a proven script element's src is later removed",
            });
        }
        match resolver.static_string_at_sink(value, 0, sink_node_id) {
            Some(url) if !url.trim().is_empty() => {
                if !candidates.contains(&url) {
                    candidates.push(url);
                }
            }
            _ => dynamic = true,
        }
    }

    // Some launchers have to use document.write because they are themselves
    // parser-inserted scripts. Accept only the small, fully reconstructed
    // markup form below: every fragment must be static except a current
    // script's nonce, and the resulting text must be exactly one script tag.
    for write in &writes {
        if !resolver.is_document_object(write.object, 0) {
            continue;
        }
        if resolver.sink_block(write.node_id).is_none() {
            dynamic = true;
            continue;
        }
        match resolver.written_script_src(write.value, 0, write.node_id) {
            Some(Some(url)) => {
                saw_script_element = true;
                if !candidates.contains(&url) {
                    candidates.push(url);
                }
            }
            Some(None) => dynamic = true,
            None => dynamic = true,
        }
    }

    if write_mutations
        .iter()
        .any(|object| resolver.is_document_object(object, 0))
    {
        dynamic = true;
    }

    if has_unproven_scope && saw_script_element {
        return Err(Error::UnprovenLauncher {
            reason: "direct eval and with scopes are not statically bounded",
        });
    }

    let seeded = candidates
        .iter()
        .filter(
            |candidate| match seed::seed_from_reference(candidate, seed_param) {
                Ok(_) => true,
                Err(Error::MissingSeed { .. }) => false,
                Err(_) => false,
            },
        )
        .count();
    let invalid_seeded =
        candidates.iter().any(
            |candidate| match seed::seed_from_reference(candidate, seed_param) {
                Ok(_) | Err(Error::MissingSeed { .. }) => false,
                Err(_) => true,
            },
        );

    match (candidates.len(), seeded, dynamic || invalid_seeded) {
        (0, 0, false) => {}
        (1, 1, false) => {
            return Ok(Script::Launcher(Launcher {
                kernel_ref: candidates.remove(0),
            }))
        }
        (1, 0, false) => {
            let kernel_with_helper_suffix = resolver.is_kernel_with_trailing_helper(
                &program,
                &candidates[0],
                &sinks,
                &writes,
                bytes,
                seed_param,
            );
            if !kernel_with_helper_suffix {
                return Ok(Script::Launcher(Launcher {
                    kernel_ref: candidates.remove(0),
                }));
            }
        }
        (_, _, true) => return Err(Error::DynamicKernelUrl),
        (_, 1, false) => {
            let Some(candidate) = candidates
                .iter()
                .find(|candidate| seed::seed_from_reference(candidate, seed_param).is_ok())
                .cloned()
            else {
                return Err(Error::AmbiguousKernelUrl { candidates });
            };
            return Ok(Script::Launcher(Launcher {
                kernel_ref: candidate,
            }));
        }
        _ => return Err(Error::AmbiguousKernelUrl { candidates }),
    }

    if functions == 0 {
        return Err(Error::UnrecognizedScript {
            reason: "it loads no script and defines no function".to_owned(),
        });
    }
    if bytes.len() < MIN_KERNEL_BYTES {
        return Err(Error::UnrecognizedScript {
            reason: format!(
                "it loads no script and at {} bytes is too small to be a kernel",
                bytes.len()
            ),
        });
    }
    Ok(Script::Kernel(KernelSummary {
        bytes: bytes.len(),
        sha256: sha256_hex(bytes),
        statements: program.body.len(),
        functions,
    }))
}

struct SrcRemoval<'a> {
    element: &'a Expression<'a>,
    /// `None` is treated conservatively as an unknown attribute name.
    attribute: Option<&'a Expression<'a>>,
}

/// Classify `bytes` and require an async launcher.
pub fn parse_launcher(bytes: &[u8]) -> Result<Launcher> {
    parse_launcher_with_seed_param(bytes, DEFAULT_SEED_PARAM)
}

/// Classify `bytes` and require an async launcher using `seed_param` to
/// choose among multiple proven script loads.
pub fn parse_launcher_with_seed_param(bytes: &[u8], seed_param: &str) -> Result<Launcher> {
    match classify_with_seed_param(bytes, seed_param)? {
        Script::Launcher(launcher) => Ok(launcher),
        other => Err(Error::ExpectedLauncher {
            found: other.describe(),
        }),
    }
}

/// Classify `bytes` and require a raw kernel.
pub fn parse_kernel(bytes: &[u8]) -> Result<KernelSummary> {
    match classify(bytes)? {
        Script::Kernel(summary) => Ok(summary),
        other => Err(Error::ExpectedKernel {
            found: other.describe(),
        }),
    }
}

/// Length of the kernel prefix when a **direct kernel** response ends with a
/// final top-level IIFE (the per-request configuration dispatcher). The prefix
/// must independently classify as a raw kernel; otherwise the whole response is
/// the kernel and `None` is returned.
///
/// Only direct-kernel sites call this. An async launcher stores its bundle
/// byte-for-byte, because every statement in it is the artifact that was served.
pub fn kernel_prefix_len(bytes: &[u8]) -> Option<usize> {
    let source = std::str::from_utf8(bytes).ok()?;
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::script()).parse();
    if !parsed.errors.is_empty() || parsed.panicked {
        return None;
    }
    let program = parsed.program;
    if program.body.len() < 2 {
        return None;
    }
    let Some(oxc_ast::ast::Statement::ExpressionStatement(statement)) = program.body.last() else {
        return None;
    };
    if !is_iife_expression(&statement.expression) {
        return None;
    }
    let start = statement.span.start as usize;
    if start == 0 || start >= bytes.len() {
        return None;
    }
    matches!(classify(&bytes[..start]), Ok(Script::Kernel(_))).then_some(start)
}

/// One place a binding receives a value.
enum Definition<'a> {
    /// `var x = <expr>`
    Value {
        value: &'a Expression<'a>,
        node_id: NodeId,
    },
    /// `x = <expr>`
    Assigned {
        value: &'a Expression<'a>,
        node_id: NodeId,
    },
    /// `x += …` and friends. Only `+=` is reconstructed for bounded markup.
    Compound {
        operator: AssignmentOperator,
        right: &'a Expression<'a>,
        node_id: NodeId,
    },
}

struct WriteSite<'a> {
    object: &'a Expression<'a>,
    value: &'a Expression<'a>,
    node_id: NodeId,
}

struct Resolver<'a, 's> {
    scoping: &'s Scoping,
    definitions: &'s HashMap<SymbolId, Vec<Definition<'a>>>,
    nodes: &'s AstNodes<'a>,
}

impl<'a> Resolver<'a, '_> {
    /// The single expression a binding ever holds, if that can be shown: one
    /// initialiser or one plain assignment, and no other write of any kind
    /// (updates, destructuring, loop heads) anywhere in the program.
    fn sole_value(&self, expression: &Expression<'a>) -> Option<&'a Expression<'a>> {
        let Expression::Identifier(identifier) = expression else {
            return None;
        };
        let reference = identifier.reference_id.get()?;
        let symbol = self.scoping.get_reference(reference).symbol_id()?;
        let [definition] = self.definitions.get(&symbol)?.as_slice() else {
            return None;
        };
        let writes = self
            .scoping
            .get_resolved_references(symbol)
            .filter(|reference| reference.is_write())
            .count();
        match definition {
            Definition::Value { value, .. } if writes == 0 => Some(value),
            Definition::Assigned { value, .. } if writes == 1 => Some(value),
            _ => None,
        }
    }

    /// Return the binding writes in source order only when they form one
    /// straight-line reconstruction ending at `sink_node_id`. A compound
    /// reconstruction is intentionally local to the sink's block and
    /// function; this rejects branch joins, loop/closure writes, and writes
    /// before initialization or after the sink.
    fn markup_definitions(
        &self,
        symbol: SymbolId,
        sink_node_id: NodeId,
    ) -> Option<Vec<&Definition<'a>>> {
        let definitions = self.definitions.get(&symbol)?;
        let writes = self
            .scoping
            .get_resolved_references(symbol)
            .filter(|reference| reference.is_write())
            .count();
        let has_initializer = definitions
            .iter()
            .any(|definition| matches!(definition, Definition::Value { .. }));
        let expected_writes = definitions.len() - usize::from(has_initializer);
        if writes != expected_writes {
            return None;
        }

        let sink_start = self.node_span_start(sink_node_id);
        let sink_block = self.sink_block(sink_node_id)?;
        let sink_function = self.enclosing_function(sink_node_id);
        let has_compound = definitions
            .iter()
            .any(|definition| matches!(definition, Definition::Compound { .. }));

        let mut ordered: Vec<&Definition<'a>> = definitions.iter().collect();
        ordered.sort_by_key(|definition| self.node_span_start(self.definition_node_id(definition)));

        if has_compound {
            let mut initialized = false;
            let mut last_start = 0;
            for definition in &ordered {
                let node_id = self.definition_node_id(definition);
                if self.definition_block(definition)? != sink_block
                    || self.enclosing_function(node_id) != sink_function
                {
                    return None;
                }
                let start = self.node_span_start(node_id);
                if start >= sink_start {
                    return None;
                }
                match definition {
                    Definition::Value { .. } if !initialized => {
                        initialized = true;
                        last_start = start;
                    }
                    Definition::Compound {
                        operator: AssignmentOperator::Addition,
                        ..
                    } if initialized && start > last_start => last_start = start,
                    _ => return None,
                }
            }
            initialized.then_some(ordered)
        } else if ordered.len() == 1 {
            let definition = ordered[0];
            self.definition_dominates_sink(definition, sink_block, sink_function, sink_start)
                .then_some(ordered)
        } else {
            None
        }
    }

    fn definition_node_id(&self, definition: &Definition<'a>) -> NodeId {
        match definition {
            Definition::Value { node_id, .. }
            | Definition::Assigned { node_id, .. }
            | Definition::Compound { node_id, .. } => *node_id,
        }
    }

    fn node_span_start(&self, node_id: NodeId) -> u32 {
        self.nodes.kind(node_id).span().start
    }

    fn definition_block(&self, definition: &Definition<'a>) -> Option<NodeId> {
        match definition {
            Definition::Value { node_id, .. } => {
                let declaration = self.nodes.parent_id(*node_id);
                matches!(
                    self.nodes.kind(declaration),
                    AstKind::VariableDeclaration(_)
                )
                .then(|| self.direct_statement_block(declaration))
                .flatten()
            }
            Definition::Assigned { node_id, .. } | Definition::Compound { node_id, .. } => {
                self.direct_expression_statement_block(*node_id)
            }
        }
    }

    fn direct_statement_block(&self, statement_id: NodeId) -> Option<NodeId> {
        let parent = self.nodes.parent_id(statement_id);
        matches!(
            self.nodes.kind(parent),
            AstKind::Program(_) | AstKind::BlockStatement(_) | AstKind::FunctionBody(_)
        )
        .then_some(parent)
    }

    fn direct_expression_statement_block(&self, expression_id: NodeId) -> Option<NodeId> {
        let mut current = expression_id;
        loop {
            let parent = self.nodes.parent_id(current);
            match self.nodes.kind(parent) {
                AstKind::ParenthesizedExpression(_) => current = parent,
                AstKind::ExpressionStatement(_) => return self.direct_statement_block(parent),
                _ => return None,
            }
        }
    }

    fn sink_block(&self, node_id: NodeId) -> Option<NodeId> {
        let mut current = node_id;
        let mut assignment = false;
        let mut call = false;
        loop {
            let parent = self.nodes.parent_id(current);
            match self.nodes.kind(parent) {
                AstKind::ParenthesizedExpression(_) => current = parent,
                AstKind::AssignmentExpression(_) if !assignment => {
                    assignment = true;
                    current = parent;
                }
                AstKind::CallExpression(_) if !call && !assignment => {
                    call = true;
                    current = parent;
                }
                AstKind::ExpressionStatement(_) => return self.direct_statement_block(parent),
                _ => return None,
            }
        }
    }

    fn block_is_ancestor(&self, ancestor: NodeId, descendant: NodeId) -> bool {
        let mut current = descendant;
        loop {
            if current == ancestor {
                return true;
            }
            let parent = self.nodes.parent_id(current);
            if parent == current {
                return false;
            }
            current = parent;
        }
    }

    fn definition_dominates_sink(
        &self,
        definition: &Definition<'a>,
        sink_block: NodeId,
        sink_function: Option<NodeId>,
        sink_start: u32,
    ) -> bool {
        let node_id = self.definition_node_id(definition);
        self.definition_block(definition)
            .is_some_and(|definition_block| {
                self.block_is_ancestor(definition_block, sink_block)
                    && self.enclosing_function(node_id) == sink_function
                    && self.node_span_start(node_id) < sink_start
            })
    }

    fn enclosing_function(&self, node_id: NodeId) -> Option<NodeId> {
        let mut current = node_id;
        loop {
            match self.nodes.kind(current) {
                AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) => return Some(current),
                _ => {
                    let parent = self.nodes.parent_id(current);
                    if parent == current {
                        return None;
                    }
                    current = parent;
                }
            }
        }
    }

    fn top_level_statement(&self, node_id: NodeId) -> Option<NodeId> {
        let mut current = node_id;
        loop {
            let parent = self.nodes.parent_id(current);
            if matches!(self.nodes.kind(parent), AstKind::Program(_)) {
                return Some(current);
            }
            if parent == current {
                return None;
            }
            current = parent;
        }
    }

    /// A response may be a raw kernel followed by one parser-inserted helper
    /// statement. The prefix must independently satisfy the existing raw
    /// kernel classifier; only a final IIFE containing one unseeded proven
    /// script load is admitted as the helper suffix.
    fn is_kernel_with_trailing_helper(
        &self,
        program: &oxc_ast::ast::Program<'a>,
        candidate: &str,
        sinks: &[(&Expression<'a>, &Expression<'a>, NodeId)],
        writes: &[WriteSite<'a>],
        bytes: &[u8],
        seed_param: &str,
    ) -> bool {
        if writes
            .iter()
            .any(|write| self.is_document_object(write.object, 0))
        {
            return false;
        }
        let matching: Vec<NodeId> = sinks
            .iter()
            .filter_map(|(element, value, member_node_id)| {
                (self.is_script_element(element, 0)
                    && self.static_string(value, 0).as_deref() == Some(candidate))
                .then(|| self.top_level_statement(*member_node_id))
                .flatten()
            })
            .collect();
        let Some(&helper_id) = matching.first() else {
            return false;
        };
        if matching.len() != 1
            || program.body.len() < 2
            || program
                .body
                .last()
                .is_none_or(|statement| statement.span() != self.nodes.kind(helper_id).span())
        {
            return false;
        }
        let AstKind::ExpressionStatement(statement) = self.nodes.kind(helper_id) else {
            return false;
        };
        if !is_iife_expression(&statement.expression) {
            return false;
        }
        let helper_start = self.node_span_start(helper_id) as usize;
        if helper_start < MIN_KERNEL_BYTES || helper_start > bytes.len() {
            return false;
        }
        matches!(
            classify_with_seed_param(&bytes[..helper_start], seed_param),
            Ok(Script::Kernel(_))
        )
    }

    fn markup_string(
        &self,
        expression: &Expression<'a>,
        depth: u8,
        sink_node_id: NodeId,
    ) -> Option<Vec<MarkupPart>> {
        if depth > MAX_RESOLVE_DEPTH {
            return None;
        }
        match expression.get_inner_expression() {
            Expression::StringLiteral(literal) => {
                Some(vec![MarkupPart::Known(literal.value.to_string())])
            }
            Expression::TemplateLiteral(template) if template.expressions.is_empty() => {
                let [quasi] = template.quasis.as_slice() else {
                    return None;
                };
                Some(vec![MarkupPart::Known(quasi.value.cooked?.to_string())])
            }
            Expression::BinaryExpression(binary) if binary.operator == BinaryOperator::Addition => {
                let mut parts = self.markup_string(&binary.left, depth + 1, sink_node_id)?;
                let right = self.markup_string(&binary.right, depth + 1, sink_node_id)?;
                parts.extend(right);
                Some(parts)
            }
            inner @ Expression::Identifier(_) => {
                if self.is_current_script_nonce(inner, depth + 1) {
                    return Some(vec![MarkupPart::Nonce]);
                }
                let identifier = inner.get_inner_expression();
                let Expression::Identifier(identifier) = identifier else {
                    return None;
                };
                let reference = identifier.reference_id.get()?;
                let symbol = self.scoping.get_reference(reference).symbol_id()?;
                let definitions = self.markup_definitions(symbol, sink_node_id)?;
                let mut result = Vec::new();
                for definition in definitions {
                    match definition {
                        Definition::Value { value, .. } | Definition::Assigned { value, .. } => {
                            if !result.is_empty() {
                                return None;
                            }
                            result.extend(self.markup_string(value, depth + 1, sink_node_id)?);
                        }
                        Definition::Compound {
                            operator: AssignmentOperator::Addition,
                            right,
                            ..
                        } => {
                            result.extend(self.markup_string(right, depth + 1, sink_node_id)?);
                        }
                        Definition::Compound { .. } => return None,
                    }
                }
                Some(result)
            }
            inner if self.is_current_script_nonce(inner, depth + 1) => {
                Some(vec![MarkupPart::Nonce])
            }
            _ => None,
        }
    }

    /// Reconstruct a document.write argument and return its one quoted src,
    /// `None` for known non-script markup, or `None` when the markup is not
    /// statically bounded. The caller distinguishes the latter by the
    /// `Option<Option<String>>` result from this method.
    fn written_script_src(
        &self,
        expression: &Expression<'a>,
        depth: u8,
        sink_node_id: NodeId,
    ) -> Option<Option<String>> {
        let parts = self.markup_string(expression, depth, sink_node_id)?;
        let mut rendered = String::new();
        let nonce = "__SHAPE_CURRENT_SCRIPT_NONCE__";
        for part in parts {
            match part {
                MarkupPart::Known(value) => rendered.push_str(&value),
                MarkupPart::Nonce => rendered.push_str(nonce),
            }
        }
        parse_script_markup(&rendered, nonce).ok()
    }

    fn static_string(&self, expression: &Expression<'a>, depth: u8) -> Option<String> {
        if depth > MAX_RESOLVE_DEPTH {
            return None;
        }
        match expression.get_inner_expression() {
            Expression::StringLiteral(literal) => Some(literal.value.to_string()),
            Expression::TemplateLiteral(template) if template.expressions.is_empty() => {
                let [quasi] = template.quasis.as_slice() else {
                    return None;
                };
                quasi.value.cooked.map(|cooked| cooked.to_string())
            }
            Expression::BinaryExpression(binary) if binary.operator == BinaryOperator::Addition => {
                let mut left = self.static_string(&binary.left, depth + 1)?;
                left.push_str(&self.static_string(&binary.right, depth + 1)?);
                Some(left)
            }
            inner @ Expression::Identifier(_) => {
                self.static_string(self.sole_value(inner)?, depth + 1)
            }
            _ => None,
        }
    }

    fn static_string_at_sink(
        &self,
        expression: &Expression<'a>,
        depth: u8,
        sink_node_id: NodeId,
    ) -> Option<String> {
        if depth > MAX_RESOLVE_DEPTH {
            return None;
        }
        match expression.get_inner_expression() {
            Expression::StringLiteral(literal) => Some(literal.value.to_string()),
            Expression::TemplateLiteral(template) if template.expressions.is_empty() => {
                let [quasi] = template.quasis.as_slice() else {
                    return None;
                };
                quasi.value.cooked.map(|cooked| cooked.to_string())
            }
            Expression::BinaryExpression(binary) if binary.operator == BinaryOperator::Addition => {
                let mut left = self.static_string_at_sink(&binary.left, depth + 1, sink_node_id)?;
                left.push_str(&self.static_string_at_sink(
                    &binary.right,
                    depth + 1,
                    sink_node_id,
                )?);
                Some(left)
            }
            Expression::Identifier(identifier) => {
                let reference = identifier.reference_id.get()?;
                let symbol = self.scoping.get_reference(reference).symbol_id()?;
                let [definition] = self.definitions.get(&symbol)?.as_slice() else {
                    return None;
                };
                let writes = self
                    .scoping
                    .get_resolved_references(symbol)
                    .filter(|reference| reference.is_write())
                    .count();
                let value = match definition {
                    Definition::Value { value, .. } if writes == 0 => value,
                    Definition::Assigned { value, .. } if writes == 1 => value,
                    _ => return None,
                };
                let sink_block = self.sink_block(sink_node_id)?;
                let sink_function = self.enclosing_function(sink_node_id);
                let dominates = self.definition_dominates_sink(
                    definition,
                    sink_block,
                    sink_function,
                    self.node_span_start(sink_node_id),
                );
                dominates.then(|| self.static_string_at_sink(value, depth + 1, sink_node_id))?
            }
            _ => self.static_string(expression, depth),
        }
    }

    /// Does `expression` provably evaluate to a freshly created script element?
    fn is_script_element(&self, expression: &Expression<'a>, depth: u8) -> bool {
        if depth > MAX_RESOLVE_DEPTH {
            return false;
        }
        match expression.get_inner_expression() {
            Expression::CallExpression(call) => {
                let creates = call
                    .callee
                    .get_inner_expression()
                    .as_member_expression()
                    .is_some_and(|member| {
                        member.static_property_name() == Some("createElement")
                            && self.is_document_object(member.object(), depth + 1)
                    });
                let tag = call
                    .arguments
                    .first()
                    .and_then(Argument::as_expression)
                    .and_then(|argument| self.static_string(argument, depth + 1));
                creates
                    && call.arguments.len() == 1
                    && tag.is_some_and(|tag| tag.eq_ignore_ascii_case("script"))
            }
            inner @ Expression::Identifier(_) => self
                .sole_value(inner)
                .is_some_and(|value| self.is_script_element(value, depth + 1)),
            _ => false,
        }
    }

    fn is_script_element_at_sink(
        &self,
        expression: &Expression<'a>,
        depth: u8,
        sink_node_id: NodeId,
    ) -> bool {
        if depth > MAX_RESOLVE_DEPTH {
            return false;
        }
        match expression.get_inner_expression() {
            Expression::CallExpression(call) => {
                let creates = call
                    .callee
                    .get_inner_expression()
                    .as_member_expression()
                    .is_some_and(|member| {
                        member.static_property_name() == Some("createElement")
                            && self.is_document_object(member.object(), depth + 1)
                    });
                let tag = call
                    .arguments
                    .first()
                    .and_then(Argument::as_expression)
                    .and_then(|argument| self.static_string(argument, depth + 1));
                creates
                    && call.arguments.len() == 1
                    && tag.is_some_and(|tag| tag.eq_ignore_ascii_case("script"))
            }
            Expression::Identifier(identifier) => {
                let Some(reference) = identifier.reference_id.get() else {
                    return false;
                };
                let Some(symbol) = self.scoping.get_reference(reference).symbol_id() else {
                    return false;
                };
                let Some([definition]) = self.definitions.get(&symbol).map(Vec::as_slice) else {
                    return false;
                };
                let writes = self
                    .scoping
                    .get_resolved_references(symbol)
                    .filter(|reference| reference.is_write())
                    .count();
                let Some(value) = (match definition {
                    Definition::Value { value, .. } if writes == 0 => Some(*value),
                    Definition::Assigned { value, .. } if writes == 1 => Some(*value),
                    _ => None,
                }) else {
                    return false;
                };
                let Some(sink_block) = self.sink_block(sink_node_id) else {
                    return false;
                };
                self.definition_dominates_sink(
                    definition,
                    sink_block,
                    self.enclosing_function(sink_node_id),
                    self.node_span_start(sink_node_id),
                ) && self.is_script_element_at_sink(value, depth + 1, sink_node_id)
            }
            _ => false,
        }
    }

    /// Return the canonical binding for an element expression. This follows
    /// aliases by SymbolId rather than comparing source-level names, so a
    /// later mutation through any statically proven alias invalidates the
    /// candidate.
    fn element_binding(&self, expression: &Expression<'a>, depth: u8) -> Option<SymbolId> {
        if depth > MAX_RESOLVE_DEPTH {
            return None;
        }
        let Expression::Identifier(identifier) = expression.get_inner_expression() else {
            return None;
        };
        let reference = identifier.reference_id.get()?;
        let symbol = self.scoping.get_reference(reference).symbol_id()?;
        match self.sole_value(expression) {
            Some(value @ Expression::Identifier(_)) => self.element_binding(value, depth + 1),
            _ => Some(symbol),
        }
    }

    fn same_element(&self, left: &Expression<'a>, right: &Expression<'a>, depth: u8) -> bool {
        let left = self.element_binding(left, depth);
        left.is_some() && left == self.element_binding(right, depth)
    }

    fn removes_src(&self, removal: &SrcRemoval<'a>) -> bool {
        removal.attribute.is_none_or(|attribute| {
            self.static_string(attribute, 0)
                .is_none_or(|name| name.eq_ignore_ascii_case("src"))
        })
    }

    /// `document.createElement` is the browser loading primitive used by the
    /// supported launcher protocol. Aliases are accepted only when their
    /// binding is a single, statically known reference to the global
    /// `document`; an unrelated object's `createElement` method is not enough
    /// evidence to classify a file as a launcher.
    fn is_document_object(&self, expression: &Expression<'a>, depth: u8) -> bool {
        if depth > MAX_RESOLVE_DEPTH {
            return false;
        }
        match expression.get_inner_expression() {
            Expression::Identifier(identifier) => {
                let Some(reference) = identifier.reference_id.get() else {
                    return identifier.name == "document";
                };
                let Some(_symbol) = self.scoping.get_reference(reference).symbol_id() else {
                    return identifier.name == "document";
                };
                // Any symbol named `document` is a local shadow, including a
                // function parameter or a top-level declaration. Only an
                // alias with a different name may resolve to the browser
                // global below.
                if identifier.name == "document" {
                    return false;
                }
                self.sole_value(expression)
                    .is_some_and(|value| self.is_document_object(value, depth + 1))
            }
            _ => false,
        }
    }

    fn is_current_script_nonce(&self, expression: &Expression<'a>, depth: u8) -> bool {
        if depth > MAX_RESOLVE_DEPTH {
            return false;
        }
        let expression = expression.get_inner_expression();
        if let Expression::Identifier(_) = expression {
            return self
                .sole_value(expression)
                .is_some_and(|value| self.is_current_script_nonce(value, depth + 1));
        }
        let Expression::ConditionalExpression(conditional) = expression else {
            return is_current_script_member(expression, self, depth + 1, "nonce");
        };
        self.static_string(&conditional.alternate, depth + 1)
            .is_some_and(|value| value.is_empty())
            && is_current_script_member(&conditional.consequent, self, depth + 1, "nonce")
    }
}

enum MarkupPart {
    Known(String),
    Nonce,
}

fn is_iife_expression(expression: &Expression<'_>) -> bool {
    match expression.get_inner_expression() {
        Expression::CallExpression(call) => matches!(
            call.callee.get_inner_expression(),
            Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_)
        ),
        Expression::UnaryExpression(unary) => is_iife_expression(&unary.argument),
        _ => false,
    }
}

fn member_node_id(member: &oxc_ast::ast::MemberExpression<'_>) -> NodeId {
    match member {
        oxc_ast::ast::MemberExpression::ComputedMemberExpression(member) => member.node_id(),
        oxc_ast::ast::MemberExpression::StaticMemberExpression(member) => member.node_id(),
        oxc_ast::ast::MemberExpression::PrivateFieldExpression(member) => member.node_id(),
    }
}

fn is_current_script_member<'a>(
    expression: &Expression<'a>,
    resolver: &Resolver<'a, '_>,
    depth: u8,
    property: &str,
) -> bool {
    let Some(member) = expression.get_inner_expression().as_member_expression() else {
        return false;
    };
    member.static_property_name() == Some(property)
        && resolver.is_current_script(member.object(), depth + 1)
}

impl<'a> Resolver<'a, '_> {
    fn is_current_script(&self, expression: &Expression<'a>, depth: u8) -> bool {
        if depth > MAX_RESOLVE_DEPTH {
            return false;
        }
        let expression = expression.get_inner_expression();
        if let Expression::Identifier(_) = expression {
            return self
                .sole_value(expression)
                .is_some_and(|value| self.is_current_script(value, depth + 1));
        }
        let Some(member) = expression.as_member_expression() else {
            return false;
        };
        member.static_property_name() == Some("currentScript")
            && self.is_document_object(member.object(), depth + 1)
    }
}

/// Parse only the markup shape emitted by the bounded document.write path.
/// Attribute values are quoted, names are compared as HTML does (ASCII
/// case-insensitively), the source attribute is unique, and no text outside
/// the one script element is accepted. The nonce marker is allowed only as
/// the complete value of one `nonce` attribute. This models a browser DOM
/// current-script nonce as a trusted CSP nonce token; arbitrary dynamic text
/// is never interpolated into markup.
fn parse_script_markup(input: &str, nonce_marker: &str) -> std::result::Result<Option<String>, ()> {
    let Some(rest) = input.strip_prefix("<script") else {
        return Ok(None);
    };
    if !rest.chars().next().is_some_and(char::is_whitespace) {
        return Ok(None);
    }
    let end = rest.find('>').ok_or(())?;
    let mut attributes = rest[..end].trim();
    let mut src = None;
    let mut nonce_count = 0;
    while !attributes.is_empty() {
        attributes = attributes.trim_start();
        if attributes.is_empty() {
            break;
        }
        let name_end = attributes
            .find(|character: char| character == '=' || character.is_ascii_whitespace())
            .ok_or(())?;
        let name = &attributes[..name_end];
        if name.is_empty()
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        {
            return Err(());
        }
        let name = name.to_ascii_lowercase();
        if !matches!(name.as_str(), "src" | "id" | "nonce") {
            return Err(());
        }
        attributes = attributes[name_end..].trim_start();
        attributes = attributes.strip_prefix('=').ok_or(())?.trim_start();
        let quoted = attributes.strip_prefix('"').ok_or(())?;
        let value_end = quoted.find('"').ok_or(())?;
        let value = decode_html_attribute_value(&quoted[..value_end]).ok_or(())?;
        attributes = &quoted[value_end + 1..];
        if value.contains(nonce_marker) {
            if name != "nonce" || value != nonce_marker {
                return Err(());
            }
            nonce_count += 1;
        }
        if name == "src" && src.replace(value).is_some() {
            return Err(());
        }
    }
    let close = &rest[end + 1..];
    let close = close.strip_prefix("</script>").ok_or(())?;
    if !close.is_empty() || nonce_count > 1 {
        return Err(());
    }
    Ok(src)
}

/// Decode the small, unambiguous subset of HTML attribute references needed
/// by launcher URLs. Unknown or malformed references fail closed. Ordinary
/// URL query ampersands without a reference-looking suffix remain literal, as
/// they do in the browser's HTML tokenizer.
fn decode_html_attribute_value(input: &str) -> Option<String> {
    let mut output = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(ampersand) = rest.find('&') {
        output.push_str(&rest[..ampersand]);
        rest = &rest[ampersand + 1..];
        let next_ampersand = rest.find('&');
        let semicolon = rest.find(';');
        let Some(end) = semicolon
            .filter(|end| next_ampersand.is_none_or(|next_ampersand| *end < next_ampersand))
        else {
            let token_end = next_ampersand.unwrap_or(rest.len());
            if token_end == 0 {
                return None;
            }
            if is_unterminated_entity_like(&rest[..token_end]) {
                return None;
            }
            output.push('&');
            output.push_str(&rest[..token_end]);
            rest = &rest[token_end..];
            continue;
        };
        let entity = &rest[..end];
        let decoded = match entity {
            "amp" => "&".to_owned(),
            "lt" => "<".to_owned(),
            "gt" => ">".to_owned(),
            "quot" => "\"".to_owned(),
            "apos" => "'".to_owned(),
            _ if entity.starts_with("#x") || entity.starts_with("#X") => {
                decode_numeric_reference(&entity[2..], 16)?
            }
            _ if entity.starts_with('#') => decode_numeric_reference(&entity[1..], 10)?,
            _ if entity.is_empty() => return None,
            _ => return None,
        };
        output.push_str(&decoded);
        rest = &rest[end + 1..];
    }
    output.push_str(rest);
    Some(output)
}

/// Conservatively reject unterminated references that could be an HTML legacy
/// name. This is lexical only; it deliberately does not claim to implement the
/// full HTML named-character-reference table. Unknown query parameters such as
/// `&x=1`, `&token=ABC`, and GEICO's `&X-GyJwza5Z--z=q` remain literal.
fn is_unterminated_entity_like(token: &str) -> bool {
    if token.starts_with('#') {
        return true;
    }
    let bytes = token.as_bytes();
    if !bytes.first().is_some_and(u8::is_ascii_alphabetic) {
        return false;
    }
    let prefix_len = bytes
        .iter()
        .position(|byte| !byte.is_ascii_alphanumeric())
        .unwrap_or(bytes.len());
    prefix_len >= 2 && bytes.get(prefix_len) != Some(&b'=')
}

fn decode_numeric_reference(digits: &str, radix: u32) -> Option<String> {
    if digits.is_empty()
        || !digits.bytes().all(|byte| match radix {
            10 => byte.is_ascii_digit(),
            16 => byte.is_ascii_hexdigit(),
            _ => false,
        })
    {
        return None;
    }
    let value = u32::from_str_radix(digits, radix).ok()?;
    // Keep this deliberately narrower than the HTML replacement algorithm.
    // In particular, do not claim a URL for null/control, C1, surrogate, or
    // out-of-range references whose browser replacement behavior is not
    // modeled here.
    if !(0x20..=0x7e).contains(&value) && !(0xa0..=0x10ffff).contains(&value) {
        return None;
    }
    char::from_u32(value).map(|character| character.to_string())
}
