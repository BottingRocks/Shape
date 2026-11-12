//! Bounded proof for the worker-posted seeded environment bundle.
//!
//! The family carries two source-backed roles on one top-level, unreferenced
//! function declaration (the function the program posts to the worker):
//!
//! ```text
//! function bundle(payload, data) {
//!   // exactly four owned nested functions:
//!   //   writeNumber, createInternedStringWriter (+ writeInternedString),
//!   //   createKeystreamByteSource (+ nextKeystreamByte), and the emitter
//!   let key;                       // 32-bit mix of data.<string-array>, charCodeAt
//!   ...
//!   function emitter() {
//!     let word = Math.random() * 4294967296;
//!     let header = [word >>> 24, word >> 16 & 255, word >> 8 & 255, word & 255];
//!     let keystream = createKeystreamByteSource(word ^ CONSTANT);
//!     let write = createInternedStringWriter(keystream);
//!     ...
//!     write(location.origin, header, cursor);
//!     write(navigator.userAgent, header, cursor);
//!     write(navigator.platform, header, cursor);
//!     writeNumber(keystream, countOwnPropertiesAlongPrototypeChain(self), header, cursor);
//!     writeNumber(keystream, navigator.hardwareConcurrency, header, cursor);
//!     postMessage({a: payload, b: [[], header]});
//!   }
//!   emitter(data.<payload-bundle>);
//! }
//! ```
//!
//! The proof combines owned-body checks with ordered local header/sink origins.
//! A nonce binding may become the seeded byte source after header construction;
//! a string writer may become the cursor returned by its final invocation.
//! A byte source may also become the final prototype-count write's cursor,
//! consumed exactly once as the string writer's position, with no later reads.
//! Every write in these phases is accounted for and every callable protocol use
//! must read its proven factory result. Posted object/pair holders stay strict.
//!
//! The proof requires:
//!
//! * the container is a top-level function declaration that is never referenced
//!   in the script, has two plain and unwritten formals, and owns exactly four
//!   nested functions: the already accepted `writeNumber`,
//!   `createInternedStringWriter` and `createKeystreamByteSource` roles plus one
//!   independently proven emitter;
//! * the container derives every write of one captured 32-bit key from the
//!   second formal's string array (`charCodeAt`) and passes that binding into
//!   the keystream factory; it calls the emitter exactly once with a member of
//!   the second formal;
//! * the emitter has no formals and exactly one owned nested
//!   `countOwnPropertiesAlongPrototypeChain` helper, draws the exact
//!   `Math.random() * 4294967296` word, builds the four shifted/masked header
//!   lanes from it, seeds the keystream through `word ^ numeric`, creates the
//!   interned-string writer from that keystream, writes
//!   `location.origin` / `navigator.userAgent` / `navigator.platform` through
//!   that writer, writes `countOwnPropertiesAlongPrototypeChain(self)` and
//!   `navigator.hardwareConcurrency` through the numeric writer, writes the
//!   literal `keystream() ^ numeric` tag when the concurrency value is null,
//!   and posts the captured payload plus `[[], header]` through the unbound
//!   `postMessage` sink.
//!
//! The already accepted helper roles are reused from the shared pass and are
//! never re-proposed or relaxed. Only the two new names are emitted, and only
//! when every earlier pass left the site unclaimed. No name, span, hash, vendor,
//! thread id or generated spelling participates.
//!
//! Composition with the shared helper proofs: the complete numeric encoder is
//! proved by the accepted `thread_posthost_numeric` `writeNumber` evaluator and
//! the complete interned-string encoder by `function_finder::interned_string_writer`
//! / `shared_scoped_role`. This family does not re-catalogue those bodies. It
//! proves the bundle-specific obligations instead: the helper is exactly the
//! shared accepted role at that child span, the wrapper's typed-array ports are
//! the captures that accepted numeric body consumes, the writer factory returns
//! that accepted writer, and every key, header, writer, numeric field, cursor
//! epoch, payload pair and postMessage sink remains ordered and accounted for.

use super::*;
use oxc_semantic::SymbolId;
use oxc_span::{GetSpan, Span};

type Symbol = SymbolId;

const EMITTER_NAME: &str = "emitSeededWorkerEnvironmentBundle";
const CONTAINER_NAME: &str = "createSeededWorkerEnvironmentBundle";
const EVALUATOR: &str = "worker_environment_bundle";

/// `Math.random()` is scaled by 2^32 before the unsigned coercion.
const UINT32_SPAN: f64 = 4_294_967_296.0;

// ===========================================================================
// Small typed views of source operations
// ===========================================================================

pub(super) fn symbol(model: &Model<'_, '_>, expression: &Expression<'_>) -> Option<Symbol> {
    crate::analysis::expression_symbol(expression.without_parentheses(), model.scoping)
}

fn numeric(expression: &Expression<'_>) -> Option<f64> {
    match expression.without_parentheses() {
        Expression::NumericLiteral(literal) => Some(literal.value),
        Expression::UnaryExpression(unary) if unary.operator.as_str() == "-" => {
            numeric(&unary.argument).map(|value| -value)
        }
        _ => None,
    }
}

fn is_numeric(expression: &Expression<'_>, expected: f64) -> bool {
    numeric(expression) == Some(expected)
}

fn is_zero(expression: &Expression<'_>) -> bool {
    is_numeric(expression, 0.0)
}

pub(super) fn path_is(model: &Model<'_, '_>, expression: &Expression<'_>, path: &[&str]) -> bool {
    let actual = expression_path(model, expression.without_parentheses());
    actual.len() == path.len()
        && actual
            .iter()
            .zip(path)
            .all(|(actual, want)| actual.as_str() == *want)
}

pub(super) fn call_of<'a>(expression: &'a Expression<'a>) -> Option<&'a CallExpression<'a>> {
    let Expression::CallExpression(call) = expression.without_parentheses() else {
        return None;
    };
    (!call.optional).then_some(call)
}

fn array_items<'a>(expression: &'a Expression<'a>) -> Option<Vec<&'a Expression<'a>>> {
    let Expression::ArrayExpression(array) = expression.without_parentheses() else {
        return None;
    };
    let mut items = Vec::new();
    for element in &array.elements {
        match element {
            ArrayExpressionElement::Elision(_) | ArrayExpressionElement::SpreadElement(_) => {
                return None;
            }
            _ => items.push(element.as_expression()?),
        }
    }
    Some(items)
}

fn declared_symbol(model: &Model<'_, '_>, declarator: &VariableDeclarator<'_>) -> Option<Symbol> {
    let BindingPattern::BindingIdentifier(identifier) = &declarator.id else {
        return None;
    };
    identifier.symbol_id.get()
}

pub(super) fn assignment_target_symbol(
    model: &Model<'_, '_>,
    target: &AssignmentTarget<'_>,
) -> Option<Symbol> {
    let AssignmentTarget::AssignmentTargetIdentifier(identifier) = target else {
        return None;
    };
    crate::analysis::identifier_symbol(identifier, model.scoping)
}

pub(super) fn owned(
    model: &Model<'_, '_>,
    node_id: oxc_semantic::NodeId,
    site: FunctionSite,
) -> bool {
    model
        .owning_site(node_id)
        .is_some_and(|owner| owner.span == site.span)
}

/// The one canonical role of a site, refusing disagreement and inherited
/// structural labels. This mirrors the shared `role` contract for a span.
fn role_at<'p>(proposals: &'p BTreeMap<u32, Vec<Proposal>>, start: u32) -> Option<&'p str> {
    let claims = proposals.get(&start)?;
    let first = claims.first()?;
    (claims
        .iter()
        .all(|p| p.name == first.name && p.evaluator != "inherited_structural_label"))
    .then_some(first.name.as_str())
}

fn direct_children(model: &Model<'_, '_>, container: FunctionSite) -> Vec<FunctionSite> {
    let mut children = Vec::new();
    for site in model.all_sites() {
        if site.span == container.span || site.binding_symbol.is_none() {
            continue;
        }
        if model
            .enclosing_function(site.node_id)
            .is_some_and(|parent| parent.span == container.span)
        {
            children.push(*site);
        }
    }
    children
}

fn plain_parameters(model: &Model<'_, '_>, function: &Function<'_>, count: usize) -> bool {
    function.params.rest.is_none()
        && function.params.items.len() == count
        && function.params.items.iter().all(|parameter| {
            parameter.initializer.is_none()
                && matches!(&parameter.pattern, BindingPattern::BindingIdentifier(identifier)
                    if identifier.symbol_id.get().is_some_and(|symbol|
                        model.binding_writes(symbol) == 0
                        && model.scoping.symbol_redeclarations(symbol).is_empty()))
        })
}

// ===========================================================================
// The seeded emitter body
// ===========================================================================

struct EmitterProof {
    keystream_factory: FunctionSite,
    writer_factory: FunctionSite,
    number_writer: FunctionSite,
}

fn is_random_product(model: &Model<'_, '_>, expression: &Expression<'_>) -> bool {
    let expression = expression.without_parentheses();
    let Expression::BinaryExpression(product) = expression else {
        return false;
    };
    if product.operator.as_str() != "*" {
        return false;
    }
    let random = if is_numeric(&product.left, UINT32_SPAN) {
        &product.right
    } else if is_numeric(&product.right, UINT32_SPAN) {
        &product.left
    } else {
        return false;
    };
    let Some(call) = call_of(random) else {
        return false;
    };
    call.arguments.is_empty() && path_is(model, &call.callee, &["Math", "random"])
}

fn is_random_word(model: &Model<'_, '_>, expression: &Expression<'_>) -> bool {
    let expression = expression.without_parentheses();
    if is_random_product(model, expression) {
        return true;
    }
    let Expression::BinaryExpression(binary) = expression else {
        return false;
    };
    matches!(binary.operator.as_str(), "|" | ">>>")
        && is_zero(&binary.right)
        && is_random_product(model, &binary.left)
}

fn word_value(model: &Model<'_, '_>, expression: &Expression<'_>, random: Symbol) -> bool {
    if symbol(model, expression) == Some(random) {
        return true;
    }
    let Expression::BinaryExpression(binary) = expression.without_parentheses() else {
        return false;
    };
    matches!(binary.operator.as_str(), "|" | ">>>")
        && is_zero(&binary.right)
        && symbol(model, &binary.left) == Some(random)
}

/// Exact big-endian bytes: unsigned high byte, masked middle/low bytes.
fn byte_lane(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    random: Symbol,
    shift: u32,
) -> bool {
    let expression = expression.without_parentheses();
    let (shifted, masked) = if let Expression::BinaryExpression(mask) = expression {
        if mask.operator.as_str() == "&" && is_numeric(&mask.right, 255.0) {
            (mask.left.without_parentheses(), true)
        } else if shift == 24 {
            (expression, false)
        } else {
            return false;
        }
    } else {
        return false;
    };
    if shift == 0 {
        return word_value(model, shifted, random);
    }
    let Expression::BinaryExpression(binary) = shifted else {
        return false;
    };
    (binary.operator.as_str() == ">>>"
        || ((shift != 24 || masked) && binary.operator.as_str() == ">>"))
        && is_numeric(&binary.right, f64::from(shift))
        && word_value(model, &binary.left, random)
}

/// `(word | 0) ^ numeric` (either operand order), the keystream seed.
fn is_seeded_argument(model: &Model<'_, '_>, expression: &Expression<'_>, random: Symbol) -> bool {
    fn reads_word(model: &Model<'_, '_>, expression: &Expression<'_>, random: Symbol) -> bool {
        let expression = expression.without_parentheses();
        if symbol(model, expression) == Some(random) {
            return true;
        }
        let Expression::BinaryExpression(binary) = expression else {
            return false;
        };
        matches!(binary.operator.as_str(), "|" | ">>>")
            && is_zero(&binary.right)
            && symbol(model, &binary.left) == Some(random)
    }
    let expression = expression.without_parentheses();
    let Expression::BinaryExpression(xor) = expression else {
        return false;
    };
    if xor.operator.as_str() != "^" {
        return false;
    }
    (reads_word(model, &xor.left, random) && numeric(&xor.right).is_some())
        || (reads_word(model, &xor.right, random) && numeric(&xor.left).is_some())
}

// ===========================================================================
// The top-level bundle container
// ===========================================================================

/// Every write of `key` reads the second formal's string array through
/// `charCodeAt`.
fn expression_reads_bundle_array(
    model: &Model<'_, '_>,
    container: FunctionSite,
    expression: &Expression<'_>,
) -> bool {
    let span = expression.span();
    let mut char_code_at = false;
    let mut bundle = false;
    for node in model.nodes.iter() {
        let node_span = node.span();
        if node_span.start < span.start || node_span.end > span.end {
            continue;
        }
        match node.kind() {
            AstKind::CallExpression(call) => {
                if static_member_name(&call.callee) == Some("charCodeAt") {
                    char_code_at = true;
                }
            }
            AstKind::IdentifierReference(identifier) => {
                if let Some(symbol) = crate::analysis::identifier_symbol(identifier, model.scoping)
                {
                    if model
                        .formal_symbol_slot(symbol)
                        .is_some_and(|slot| slot.owner == container.node_id && slot.index == 1)
                    {
                        bundle = true;
                    }
                }
            }
            _ => {}
        }
    }
    char_code_at && bundle
}

/// The keystream factory captures exactly one container key binding, and every
/// write of that binding in the container is rooted in the bundle string array.
fn key_flows_from_bundle(
    model: &Model<'_, '_>,
    container: FunctionSite,
    factory: FunctionSite,
) -> bool {
    let mut captured = BTreeSet::new();
    for node in model.nodes.iter() {
        if !owned(model, node.id(), factory) {
            continue;
        }
        let value = match node.kind() {
            AstKind::VariableDeclarator(declarator) => {
                let Some(init) = declarator.init.as_ref() else {
                    continue;
                };
                init
            }
            AstKind::AssignmentExpression(assignment) if assignment.operator.as_str() == "=" => {
                &assignment.right
            }
            _ => continue,
        };
        let Some(value) = symbol(model, value) else {
            continue;
        };
        let declaration = model.scoping.symbol_declaration(value);
        if model
            .owning_site(declaration)
            .is_some_and(|owner| owner.span == container.span)
        {
            captured.insert(value);
        }
    }
    if captured.len() != 1 {
        return false;
    }
    let key = *captured.iter().next().expect("one captured key");
    let mut seen = false;
    for node in model.nodes.iter() {
        if !owned(model, node.id(), container) {
            continue;
        }
        let (target, value) = match node.kind() {
            AstKind::VariableDeclarator(declarator) => {
                let Some(init) = declarator.init.as_ref() else {
                    continue;
                };
                (declared_symbol(model, declarator), init)
            }
            AstKind::AssignmentExpression(assignment) if assignment.operator.as_str() == "=" => (
                assignment_target_symbol(model, &assignment.left),
                &assignment.right,
            ),
            _ => continue,
        };
        if target != Some(key) {
            continue;
        }
        seen = true;
        if !expression_reads_bundle_array(model, container, value) {
            return false;
        }
    }
    seen
}

struct BundleProof {
    emitter: FunctionSite,
}

fn bundle_proof(
    model: &Model<'_, '_>,
    container: FunctionSite,
    proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Option<BundleProof> {
    let AstKind::Function(function) = model.nodes.kind(container.node_id) else {
        return None;
    };
    if function.r#async
        || function.generator
        || function.r#type != FunctionType::FunctionDeclaration
    {
        return None;
    }
    if !matches!(
        model.nodes.parent_kind(container.node_id),
        AstKind::Program(_)
    ) {
        return None;
    }
    if !plain_parameters(model, function, 2) {
        return None;
    }
    let binding = container.binding_symbol?;
    if model.binding_references(binding) != 0 {
        return None;
    }
    let body = function.body.as_ref()?;
    if !body.directives.is_empty() {
        return None;
    }
    let children = direct_children(model, container);
    if children.len() != 4 {
        return None;
    }
    let mut keystream_factory = None;
    let mut writer_factory = None;
    let mut number_writer = None;
    let mut emitter = None;
    for child in &children {
        match role_at(proposals, child.span.start) {
            Some("createKeystreamByteSource") => {
                if keystream_factory.replace(*child).is_some() {
                    return None;
                }
            }
            Some("createInternedStringWriter") => {
                if writer_factory.replace(*child).is_some() {
                    return None;
                }
            }
            Some("writeNumber") => {
                if number_writer.replace(*child).is_some() {
                    return None;
                }
            }
            _ => {
                if emitter.is_some() {
                    return None;
                }
                emitter = Some(*child);
            }
        }
    }
    let keystream_factory = keystream_factory?;
    let writer_factory = writer_factory?;
    let number_writer = number_writer?;
    let emitter = emitter?;
    // Keep the pair atomic: a site already owned by another family is never
    // re-used as the emitter of this bundle, and the emitter must be referenced
    // exactly once by the owning container.
    if proposals.contains_key(&emitter.span.start) {
        return None;
    }
    let emitter_binding = emitter.binding_symbol?;
    if model.binding_references(emitter_binding) != 1 {
        return None;
    }
    let proof = closed_protocol(
        model,
        container,
        emitter,
        keystream_factory,
        writer_factory,
        number_writer,
        &children,
        proposals,
    )?;
    if proof.keystream_factory.span != keystream_factory.span
        || proof.writer_factory.span != writer_factory.span
        || proof.number_writer.span != number_writer.span
    {
        return None;
    }
    for child in &children {
        let binding = child.binding_symbol?;
        if model.binding_references(binding) == 0 {
            return None;
        }
    }
    let mut calls = 0;
    for node in model.nodes.iter() {
        if !owned(model, node.id(), container) {
            continue;
        }
        let AstKind::CallExpression(call) = node.kind() else {
            continue;
        };
        if !model
            .callee_function(&call.callee)
            .is_some_and(|target| target.span == emitter.span)
        {
            continue;
        }
        calls += 1;
        let arguments = call_arguments(call);
        let [argument] = arguments.as_slice() else {
            return None;
        };
        let Expression::StaticMemberExpression(member) = argument.without_parentheses() else {
            return None;
        };
        if member.optional || model.site_slot_value(&member.object, container) != Some(1) {
            return None;
        }
    }
    if calls != 1 {
        return None;
    }
    if !key_flows_from_bundle(model, container, keystream_factory) {
        return None;
    }
    Some(BundleProof { emitter })
}

// ===========================================================================
// Entry
// Canonical metadata describes the shared accepted helper roles and complete
// bundle protocol. Both the original numeric proof and the broader closed
// shared-role proof establish those same claims; proof implementation details
// must not migrate metadata on already-supported functions.
// ===========================================================================

pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    f: &Facts<'_>,
    proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
    // Emitter role: the enclosing function must be a complete bundle container
    // and the independently proven emitter must be this exact site.
    if f.globals.contains("postMessage") && f.members.contains("hardwareConcurrency") {
        if let Some(container) = model.enclosing_function(site.node_id) {
            if let Some(bundle) = bundle_proof(model, container, proposals) {
                if bundle.emitter.span == site.span {
                    return vec![proposal(
                        EMITTER_NAME,
                        EVALUATOR,
                        "worker-side seeded environment emitter: Math.random 32-bit header, keystream seeded from header ^ constant, interned-string/number writes of location.origin, navigator.userAgent, navigator.platform, navigator.hardwareConcurrency (literal null tag) and countOwnPropertiesAlongPrototypeChain(self), then the captured payload plus [[], header] posted through the unbound postMessage sink",
                        f,
                    )];
                }
            }
        }
    }
    // Container role: top-level, never-referenced function declaration that owns
    // the four accepted helper/emitter children and drives the bundle.
    if f.members.contains("charCodeAt") && bundle_proof(model, site, proposals).is_some() {
        return vec![proposal(
            CONTAINER_NAME,
            EVALUATOR,
            "worker-posted environment bundle container: exactly four owned nested functions (accepted writeNumber, createInternedStringWriter, createKeystreamByteSource and the independently proven emitter), a 32-bit key derived from the captured string array on formal slot 1, and one emitter call with formal slot 1 data",
            f,
        )];
    }
    Vec::new()
}
// A closed protocol interpreter. Values are identities/epochs, not JS values.
// No source spelling or function span supplies semantics. A statement succeeds
// only by executing one of the operations below; there is no presence fallback.
#[derive(Clone, Debug, PartialEq)]
enum ProtocolTerm {
    Binding(Symbol),
    Global(String),
    Number(f64),
    String(String),
    Null,
    Op(String, Vec<ProtocolTerm>),
}
use ProtocolTerm as PT;
fn pt(op: &str, args: Vec<PT>) -> PT {
    PT::Op(op.to_string(), args)
}
fn pn(n: f64) -> PT {
    PT::Number(n)
}
fn ps(s: Symbol) -> PT {
    PT::Binding(s)
}
fn pg(s: &str) -> PT {
    PT::Global(s.to_string())
}
fn pm(object: PT, name: &str) -> PT {
    pt("member", vec![object, PT::String(name.to_string())])
}
fn pb(op: &str, a: PT, b: PT) -> PT {
    pt(op, vec![a, b])
}
fn pc(callee: PT, args: Vec<PT>) -> PT {
    let mut a = vec![callee];
    a.extend(args);
    pt("call", a)
}
fn protocol_term(model: &Model<'_, '_>, e: &Expression<'_>) -> Option<PT> {
    let e = e.without_parentheses();
    Some(match e {
        Expression::Identifier(i) => symbol(model, e)
            .map(ps)
            .unwrap_or_else(|| pg(i.name.as_str())),
        Expression::NumericLiteral(n) => pn(n.value),
        Expression::StringLiteral(s) => PT::String(s.value.to_string()),
        Expression::NullLiteral(_) => PT::Null,
        Expression::BinaryExpression(b) => pb(
            b.operator.as_str(),
            protocol_term(model, &b.left)?,
            protocol_term(model, &b.right)?,
        ),
        Expression::LogicalExpression(b) => pb(
            b.operator.as_str(),
            protocol_term(model, &b.left)?,
            protocol_term(model, &b.right)?,
        ),
        Expression::UnaryExpression(u) => pt(
            u.operator.as_str(),
            vec![protocol_term(model, &u.argument)?],
        ),
        Expression::ConditionalExpression(c) => pt(
            "?:",
            vec![
                protocol_term(model, &c.test)?,
                protocol_term(model, &c.consequent)?,
                protocol_term(model, &c.alternate)?,
            ],
        ),
        Expression::StaticMemberExpression(m) if !m.optional => {
            pm(protocol_term(model, &m.object)?, m.property.name.as_str())
        }
        Expression::ComputedMemberExpression(m) if !m.optional => pt(
            "index",
            vec![
                protocol_term(model, &m.object)?,
                protocol_term(model, &m.expression)?,
            ],
        ),
        Expression::ArrayExpression(_) => pt(
            "array",
            array_items(e)?
                .into_iter()
                .map(|e| protocol_term(model, e))
                .collect::<Option<_>>()?,
        ),
        Expression::CallExpression(c) if !c.optional => {
            let args = call_arguments(c);
            if args.len() != c.arguments.len() {
                return None;
            }
            pc(
                protocol_term(model, &c.callee)?,
                args.into_iter()
                    .map(|e| protocol_term(model, e))
                    .collect::<Option<_>>()?,
            )
        }
        Expression::UpdateExpression(u) => {
            let SimpleAssignmentTarget::AssignmentTargetIdentifier(i) = &u.argument else {
                return None;
            };
            pt(
                &format!(
                    "{}{}",
                    if u.prefix { "pre" } else { "post" },
                    u.operator.as_str()
                ),
                vec![ps(crate::analysis::identifier_symbol(i, model.scoping)?)],
            )
        }
        Expression::NewExpression(c) => {
            let args = c
                .arguments
                .iter()
                .map(|a| a.as_expression().and_then(|e| protocol_term(model, e)))
                .collect::<Option<Vec<_>>>()?;
            let mut all = vec![protocol_term(model, &c.callee)?];
            all.extend(args);
            pt("new", all)
        }
        _ => return None,
    })
}
fn protocol_body<'a>(model: &Model<'_, 'a>, site: FunctionSite) -> Option<&'a FunctionBody<'a>> {
    let AstKind::Function(f) = model.nodes.kind(site.node_id) else {
        return None;
    };
    (!f.r#async && !f.generator && f.body.as_ref()?.directives.is_empty())
        .then_some(f.body.as_ref()?)
}
fn protocol_block<'a>(s: &'a Statement<'a>) -> Option<&'a [Statement<'a>]> {
    let Statement::BlockStatement(b) = s else {
        return None;
    };
    Some(&b.body)
}
fn protocol_assignment<'a>(s: &'a Statement<'a>) -> Option<&'a AssignmentExpression<'a>> {
    let Statement::ExpressionStatement(s) = s else {
        return None;
    };
    let Expression::AssignmentExpression(a) = s.expression.without_parentheses() else {
        return None;
    };
    (a.operator.as_str() == "=").then_some(a)
}
#[derive(Clone, Debug, PartialEq)]
enum ProtocolValue {
    Empty,
    Word,
    Header,
    Stream,
    Writer,
    Hardware,
    Count,
    Sink,
    Payload,
    Cursor(u8),
    PendingCursor(u8),
    Pair,
    Object(bool, bool),
    Function(u32),
}
use ProtocolValue as PV;
#[derive(Clone)]
struct EmitterState {
    values: BTreeMap<Symbol, PV>,
    fields: u8,
    cursor: u8,
    header: bool,
    paired: bool,
    posted: bool,
    count: bool,
    stream: bool,
    writer: bool,
    word: bool,
    direct_hardware_guard: bool,
}
impl EmitterState {
    fn value(&self, model: &Model<'_, '_>, e: &Expression<'_>) -> Option<PV> {
        self.values.get(&symbol(model, e)?).cloned()
    }
    fn position(&self, model: &Model<'_, '_>, e: &Expression<'_>) -> bool {
        if self.cursor == 0 {
            is_numeric(e, 3.0)
        } else {
            self.value(model, e) == Some(PV::Cursor(self.cursor))
        }
    }
    fn encode(&mut self, field: u8) -> Option<PV> {
        if !self.header || self.paired || self.posted || self.fields & field != 0 {
            return None;
        }
        self.fields |= field;
        self.cursor += 1;
        Some(PV::Cursor(self.cursor))
    }
}
struct ProtocolContext<'p> {
    container: FunctionSite,
    emitter: FunctionSite,
    count: FunctionSite,
    stream: FunctionSite,
    writer: FunctionSite,
    number: FunctionSite,
    proposals: &'p BTreeMap<u32, Vec<Proposal>>,
    captures: BTreeMap<Symbol, PV>,
}
fn protocol_callee(
    model: &Model<'_, '_>,
    ctx: &ProtocolContext<'_>,
    e: &Expression<'_>,
    expected: FunctionSite,
) -> bool {
    symbol(model, e).and_then(|s| ctx.captures.get(&s)) == Some(&PV::Function(expected.span.start))
        || symbol(model, e) == expected.binding_symbol
}
fn protocol_eval(
    model: &Model<'_, '_>,
    ctx: &ProtocolContext<'_>,
    s: &mut EmitterState,
    e: &Expression<'_>,
) -> Option<PV> {
    if s.posted {
        return None;
    }
    if let Some(v) = s.value(model, e) {
        return match v {
            PV::Count | PV::Pair | PV::Payload => Some(v),
            _ => None,
        };
    }
    if let Some(b) = symbol(model, e) {
        if ctx.captures.get(&b) == Some(&PV::Payload) {
            return Some(PV::Payload);
        }
        if Some(b) == ctx.count.binding_symbol {
            return Some(PV::Count);
        }
    }
    if is_random_word(model, e) && !s.word {
        s.word = true;
        return Some(PV::Word);
    }
    if let Expression::BinaryExpression(b) = e.without_parentheses() {
        if b.operator.as_str() == "+" && s.position(model, &b.left)
            && is_numeric(&b.right, 1.) && s.fields & 8 == 0 && !s.paired {
            return Some(PV::PendingCursor(s.cursor + 1));
        }
    }
    if path_is(model, e, &["navigator", "hardwareConcurrency"]) {
        return Some(PV::Hardware);
    }
    if path_is(model, e, &["postMessage"]) {
        return Some(PV::Sink);
    }
    if let Some(items) = array_items(e) {
        if items.len() == 4 && !s.header {
            for (&random, value) in &s.values {
                if *value == PV::Word
                    && items
                        .iter()
                        .zip([24, 16, 8, 0])
                        .all(|(e, n)| byte_lane(model, e, random, n))
                {
                    s.header = true;
                    return Some(PV::Header);
                }
            }
        }
        if items.len() == 2
            && s.fields == 31
            && !s.paired
            && !s.posted
            && array_items(items[0]).is_some_and(|a| a.is_empty())
            && s.value(model, items[1]) == Some(PV::Header)
        {
            s.paired = true;
            return Some(PV::Pair);
        }
        return None;
    }
    if let Expression::ObjectExpression(o) = e.without_parentheses() {
        let mut a = false;
        let mut b = false;
        for p in &o.properties {
            let ObjectPropertyKind::ObjectProperty(p) = p else {
                return None;
            };
            if p.computed || p.method || p.kind != PropertyKind::Init {
                return None;
            }
            match property_key_name(&p.key)?.as_str() {
                "a" if !a => {
                    if protocol_eval(model, ctx, s, &p.value)? != PV::Payload {
                        return None;
                    }
                    a = true;
                }
                "b" if !b => {
                    if protocol_eval(model, ctx, s, &p.value)? != PV::Pair {
                        return None;
                    }
                    b = true;
                }
                _ => return None,
            }
        }
        return Some(PV::Object(a, b));
    }
    let call = call_of(e)?;
    let args = call_arguments(call);
    if args.len() != call.arguments.len() {
        return None;
    }
    if protocol_callee(model, ctx, &call.callee, ctx.stream)
        && !s.stream
        && s.header
        && args.len() == 1
    {
        let random = s
            .values
            .iter()
            .find_map(|(b, v)| (*v == PV::Word).then_some(*b))?;
        if !is_seeded_argument(model, args[0], random) {
            return None;
        }
        s.stream = true;
        return Some(PV::Stream);
    }
    if protocol_callee(model, ctx, &call.callee, ctx.writer)
        && !s.writer
        && args.len() == 1
        && s.value(model, args[0]) == Some(PV::Stream)
    {
        s.writer = true;
        return Some(PV::Writer);
    }
    if s.value(model, &call.callee) == Some(PV::Writer)
        && args.len() == 3
        && s.value(model, args[1]) == Some(PV::Header)
        && s.position(model, args[2])
    {
        let field = if path_is(model, args[0], &["location", "origin"]) {
            1
        } else if path_is(model, args[0], &["navigator", "userAgent"]) {
            2
        } else if path_is(model, args[0], &["navigator", "platform"]) {
            4
        } else {
            return None;
        };
        return s.encode(field);
    }
    if protocol_callee(model, ctx, &call.callee, ctx.number)
        && args.len() == 4
        && s.value(model, args[0]) == Some(PV::Stream)
        && s.value(model, args[2]) == Some(PV::Header)
        && s.position(model, args[3])
    {
        let field = if s.value(model, args[1]) == Some(PV::Hardware)
            || (s.direct_hardware_guard && path_is(model, args[1], &["navigator", "hardwareConcurrency"])) {
            8
        } else {
            let count = call_of(args[1])?;
            let ca = call_arguments(count);
            if count.arguments.len() != 1
                || ca.len() != 1
                || !path_is(model, ca[0], &["self"])
                || !(s.value(model, &count.callee) == Some(PV::Count)
                    || symbol(model, &count.callee) == ctx.count.binding_symbol)
            {
                return None;
            }
            16
        };
        return s.encode(field);
    }
    if s.value(model, &call.callee) == Some(PV::Sink) && args.len() == 1 && s.fields == 31 {
        let object = if let Some(value) = s.value(model, args[0]) {
            value
        } else {
            protocol_eval(model, ctx, s, args[0])?
        };
        if object == PV::Object(true, true) && s.paired {
            s.posted = true;
            return Some(PV::Empty);
        }
    }
    None
}
fn protocol_store(model: &Model<'_, '_>, s: &mut EmitterState, b: Symbol, v: PV) -> Option<()> {
    if !model.scoping.symbol_redeclarations(b).is_empty() {
        return None;
    }
    if let Some(old) = s.values.get(&b) {
        let valid = matches!(
            (old, &v),
            (PV::Empty, _)
                | (PV::Word, PV::Stream)
                | (PV::Header, PV::Pair)
                | (PV::Cursor(_), PV::Cursor(_))
                | (PV::Writer, PV::Cursor(_))
                | (PV::Stream, PV::Cursor(_))
                | (PV::Hardware, PV::Cursor(_))
                | (PV::Hardware, PV::PendingCursor(_))
        );
        if !valid {
            return None;
        }
    }
    s.values.insert(b, v);
    Some(())
}
fn protocol_tag(
    model: &Model<'_, '_>,
    s: &mut EmitterState,
    a: &AssignmentExpression<'_>,
) -> Option<()> {
    let AssignmentTarget::ComputedMemberExpression(m) = &a.left else {
        return None;
    };
    if m.optional || s.value(model, &m.object) != Some(PV::Header) {
        return None;
    }
    let xor = match a.right.without_parentheses() {
        Expression::BinaryExpression(x) if x.operator.as_str() == "^" => x,
        _ => return None,
    };
    let stream_call = if is_numeric(&xor.right, 130.0) {
        call_of(&xor.left)?
    } else if is_numeric(&xor.left, 130.0) {
        call_of(&xor.right)?
    } else {
        return None;
    };
    if !stream_call.arguments.is_empty() || s.value(model, &stream_call.callee) != Some(PV::Stream)
    {
        return None;
    }
    let mut update = None;
    let index = m.expression.without_parentheses();
    if let Expression::UpdateExpression(u) = index {
        let SimpleAssignmentTarget::AssignmentTargetIdentifier(id) = &u.argument else {
            return None;
        };
        let b = crate::analysis::identifier_symbol(id, model.scoping)?;
        if !u.prefix
            || u.operator.as_str() != "++"
            || s.values.get(&b) != Some(&PV::Cursor(s.cursor))
            || s.cursor == 0
        {
            return None;
        }
        update = Some(b);
    } else if let Expression::BinaryExpression(b) = index {
        if b.operator.as_str() != "+" || !s.position(model, &b.left) || !is_numeric(&b.right, 1.0) {
            return None;
        }
    } else if let Some(binding) = symbol(model, index) {
        if s.values.get(&binding) != Some(&PV::PendingCursor(s.cursor + 1)) { return None; }
        update = Some(binding);
    } else if !(s.cursor == 0 && is_numeric(index, 4.0)) {
        return None;
    }
    let v = s.encode(8)?;
    if let Some(b) = update {
        s.values.insert(b, v);
    }
    Some(())
}
fn protocol_statements(
    model: &Model<'_, '_>,
    ctx: &ProtocolContext<'_>,
    s: &mut EmitterState,
    statements: &[Statement<'_>],
) -> Option<()> {
    for statement in statements {
        if s.posted {
            return None;
        }
        match statement {
            Statement::FunctionDeclaration(f) if f.span == ctx.count.span && !s.count => {
                s.count = true;
            }
            Statement::VariableDeclaration(d) => {
                for v in &d.declarations {
                    let b = declared_symbol(model, v)?;
                    let value = if let Some(e) = &v.init {
                        // The hardware-first spelling materializes the tag cursor 4
                        // in the null arm, after the indexed write.
                        if is_numeric(e, 4.0) && s.cursor == 1 && s.fields == 8 {
                            PV::Cursor(1)
                        } else {
                            protocol_eval(model, ctx, s, e)?
                        }
                    } else {
                        PV::Empty
                    };
                    protocol_store(model, s, b, value)?;
                }
            }
            Statement::ExpressionStatement(e) => {
                if let Expression::AssignmentExpression(a) = e.expression.without_parentheses() {
                    if a.operator.as_str() != "=" {
                        return None;
                    }
                    if let Some(b) = assignment_target_symbol(model, &a.left) {
                        let v = if is_numeric(&a.right, 4.0) && s.cursor == 1 && s.fields == 8 {
                            PV::Cursor(1)
                        } else {
                            protocol_eval(model, ctx, s, &a.right)?
                        };
                        protocol_store(model, s, b, v)?;
                    } else if let AssignmentTarget::StaticMemberExpression(m) = &a.left {
                        if m.optional {
                            return None;
                        }
                        let b = symbol(model, &m.object)?;
                        let Some(PV::Object(has_a, has_b)) = s.values.get(&b).cloned() else {
                            return None;
                        };
                        let v = protocol_eval(model, ctx, s, &a.right)?;
                        let next = match m.property.name.as_str() {
                            "a" if !has_a && v == PV::Payload => PV::Object(true, has_b),
                            "b" if !has_b && v == PV::Pair => PV::Object(has_a, true),
                            _ => return None,
                        };
                        s.values.insert(b, next);
                    } else {
                        protocol_tag(model, s, a)?;
                    }
                } else {
                    protocol_eval(model, ctx, s, &e.expression)?;
                }
            }
            Statement::IfStatement(i) => {
                let PT::Op(op, terms) = protocol_term(model, &i.test)? else {
                    return None;
                };
                if op != "==" || terms.len() != 2 || terms[1] != PT::Null {
                    return None;
                }
                let hardware = matches!(&terms[0], PT::Binding(b) if s.values.get(b) == Some(&PV::Hardware))
                    || terms[0] == pm(pg("navigator"), "hardwareConcurrency");
                if !hardware || s.fields & 8 != 0 {
                    return None;
                }
                let mut yes = s.clone();
                let mut no = s.clone();
                no.direct_hardware_guard = true;
                protocol_statements(model, ctx, &mut yes, protocol_block(&i.consequent)?)?;
                protocol_statements(model, ctx, &mut no, protocol_block(i.alternate.as_ref()?)?)?;
                if yes.fields != s.fields | 8
                    || no.fields != yes.fields
                    || yes.cursor != s.cursor + 1
                    || no.cursor != yes.cursor
                    || yes.posted
                    || no.posted
                    || yes.paired
                    || no.paired
                {
                    return None;
                }
                let mut merged = yes.clone();
                merged.direct_hardware_guard = s.direct_hardware_guard;
                for (b, v) in &mut merged.values {
                    if no.values.get(b) != Some(v) {
                        *v = PV::Empty;
                    }
                }
                *s = merged;
            }
            _ => return None,
        }
    }
    Some(())
}

#[derive(Clone, Debug, PartialEq)]
enum SetupValue {
    Payload,
    Callable(u32),
    CharacterCode,
    Uint8,
    Float64,
    Float64Guard,
    Nan,
    HashLength,
    Zero,
    Key,
}
fn protocol_formal(model: &Model<'_, '_>, site: FunctionSite, slot: usize) -> Option<Symbol> {
    let AstKind::Function(f) = model.nodes.kind(site.node_id) else {
        return None;
    };
    let BindingPattern::BindingIdentifier(id) = &f.params.items.get(slot)?.pattern else {
        return None;
    };
    id.symbol_id.get()
}
fn protocol_nan(model: &Model<'_, '_>, e: &Expression<'_>) -> bool {
    let expected = pt(
        "array",
        [0., 0., 0., 0., 0., 0., 248., 127.]
            .into_iter()
            .map(pn)
            .collect(),
    );
    if protocol_term(model, e) == Some(expected) {
        return true;
    }
    if protocol_nan_array(model, e, 0).as_deref() == Some(&[0., 0., 0., 0., 0., 0., 248., 127.]) { return true; }
    // Historical spelling: construct the same bytes through an executed IIFE.
    let Some(c) = call_of(e) else {
        return false;
    };
    let args = call_arguments(c);
    let Expression::ArrowFunctionExpression(f) = c.callee.without_parentheses() else {
        return false;
    };
    if f.r#async
        || f.expression
        || f.params.rest.is_some()
        || f.params.items.len() != 2
        || !f.body.directives.is_empty()
        || args.len() != 2
        || c.arguments.len() != 2
        || !is_numeric(args[1], 6.0)
        || protocol_term(model, args[0]) != Some(pt("array", vec![pn(0.); 6]))
    {
        return false;
    }
    let ids = f
        .params
        .items
        .iter()
        .map(|p| match &p.pattern {
            BindingPattern::BindingIdentifier(i) if p.initializer.is_none() => i.symbol_id.get(),
            _ => None,
        })
        .collect::<Option<Vec<_>>>();
    let Some(ids) = ids else {
        return false;
    };
    let [first, last, Statement::ReturnStatement(ret)] = f.body.statements.as_slice() else {
        return false;
    };
    let (Some(a), Some(b)) = (protocol_assignment(first), protocol_assignment(last)) else {
        return false;
    };
    let (
        AssignmentTarget::ComputedMemberExpression(ma),
        AssignmentTarget::ComputedMemberExpression(mb),
    ) = (&a.left, &b.left)
    else {
        return false;
    };
    !ma.optional
        && !mb.optional
        && symbol(model, &ma.object) == Some(ids[0])
        && symbol(model, &ma.expression) == Some(ids[1])
        && is_numeric(&a.right, 248.)
        && symbol(model, &mb.object) == Some(ids[0])
        && is_numeric(&mb.expression, 7.)
        && is_numeric(&b.right, 127.)
        && ret.argument.as_ref().and_then(|e| symbol(model, e)) == Some(ids[0])
}
// Evaluate only a closed array-construction grammar: literal arrays and executed
// arrow calls whose sole effects are indexed writes to their first argument.
// Argument identities, call order, every write, and the returned array are kept.
fn protocol_nan_array(model: &Model<'_, '_>, e: &Expression<'_>, depth: usize) -> Option<Vec<f64>> {
    if depth > 4 { return None; }
    if let Some(items) = array_items(e) {
        if items.len() > 8 { return None; }
        return items.iter().map(|e| numeric(e)).collect();
    }
    let c = call_of(e)?;
    let args = call_arguments(c);
    let Expression::ArrowFunctionExpression(f) = c.callee.without_parentheses() else { return None; };
    if f.r#async || f.expression || f.params.rest.is_some() || !f.body.directives.is_empty()
        || args.is_empty() || args.len() > 3 || args.len() != c.arguments.len()
        || args.len() != f.params.items.len() { return None; }
    let ids = f.params.items.iter().map(|p| {
        let BindingPattern::BindingIdentifier(id) = &p.pattern else { return None; };
        if p.initializer.is_some() || !p.decorators.is_empty() { return None; }
        let b = id.symbol_id.get()?;
        model.scoping.symbol_redeclarations(b).is_empty().then_some(b)
    }).collect::<Option<Vec<_>>>()?;
    if ids.iter().copied().collect::<BTreeSet<_>>().len() != ids.len() { return None; }
    let mut array = protocol_nan_array(model, args[0], depth + 1)?;
    let numbers = args.iter().skip(1).map(|e| numeric(e)).collect::<Option<Vec<_>>>()?;
    let scalar = |e: &Expression<'_>| numeric(e).or_else(|| {
        ids.iter().skip(1).position(|&b| symbol(model, e) == Some(b)).map(|i| numbers[i])
    });
    let (last, writes) = f.body.statements.split_last()?;
    for statement in writes {
        let a = protocol_assignment(statement)?;
        let AssignmentTarget::ComputedMemberExpression(m) = &a.left else { return None; };
        if m.optional || symbol(model, &m.object) != Some(ids[0]) { return None; }
        let index = scalar(&m.expression)?;
        if index < 0. || index >= 8. || index.fract() != 0. { return None; }
        let index = index as usize;
        // Disallow holes and writes beyond the bounded output array.
        if index > array.len() { return None; }
        let value = scalar(&a.right)?;
        if index == array.len() { array.push(value); } else { array[index] = value; }
    }
    let Statement::ReturnStatement(ret) = last else { return None; };
    if ret.argument.as_ref().and_then(|e| symbol(model, e)) != Some(ids[0]) { return None; }
    Some(array)
}
struct WrapperSetup {

    captures: BTreeMap<Symbol, PV>,
    key: Symbol,
    kinds: BTreeMap<Symbol, SetupValue>,
}
fn protocol_wrapper(
    model: &Model<'_, '_>,
    container: FunctionSite,
    emitter: FunctionSite,
    children: &[FunctionSite],
) -> Option<WrapperSetup> {
    let payload = protocol_formal(model, container, 0)?;
    let data = protocol_formal(model, container, 1)?;
    let array = pm(ps(data), "a");
    let length = pm(array.clone(), "length");
    let mut values: BTreeMap<Symbol, SetupValue> = BTreeMap::new();
    let mut declared = BTreeSet::new();
    let mut assignments: BTreeMap<Symbol, usize> = BTreeMap::new();
    let mut loop_info = None;
    let mut key = None;
    let mut own_read = false;
    let mut called = false;
    let body = protocol_body(model, container)?;
    for statement in &body.statements {
        if called {
            return None;
        }
        match statement {
            Statement::FunctionDeclaration(f) => {
                if !children.iter().any(|c| c.span == f.span) {
                    return None;
                }
            }
            Statement::ForStatement(f) => {
                if loop_info.is_some() || f.init.is_some() {
                    return None;
                }
                let PT::Op(op, terms) = protocol_term(model, f.test.as_ref()?)? else {
                    return None;
                };
                if op != "<" || terms.len() != 2 {
                    return None;
                }
                let (PT::Binding(index), PT::Binding(count)) = (&terms[0], &terms[1]) else {
                    return None;
                };
                if values.get(index) != Some(&SetupValue::Zero)
                    || values.get(count) != Some(&SetupValue::HashLength)
                {
                    return None;
                }
                let Expression::AssignmentExpression(update) =
                    f.update.as_ref()?.without_parentheses()
                else {
                    return None;
                };
                if update.operator.as_str() != "="
                    || assignment_target_symbol(model, &update.left) != Some(*index)
                    || protocol_term(model, &update.right) != Some(pb("+", ps(*index), pn(2.)))
                {
                    return None;
                }
                let [step] = protocol_block(&f.body)? else {
                    return None;
                };
                let step = protocol_assignment(step)?;
                let hash = assignment_target_symbol(model, &step.left)?;
                if values.get(&hash) != Some(&SetupValue::Zero) || hash == *index {
                    return None;
                }
                let at = |index: PT| {
                    pc(
                        pm(pt("index", vec![array.clone(), index]), "charCodeAt"),
                        vec![],
                    )
                };
                let mix = |x: PT| {
                    pb(
                        "|",
                        pb("+", pb("-", pb("<<", ps(hash), pn(5.)), ps(hash)), x),
                        pn(0.),
                    )
                };
                if protocol_term(model, &step.right)
                    != Some(mix(pb(
                        "|",
                        pb("<<", at(ps(*index)), pn(16.)),
                        at(pb("+", ps(*index), pn(1.))),
                    )))
                {
                    return None;
                }
                *assignments.entry(*index).or_default() += 1;
                *assignments.entry(hash).or_default() += 1;
                loop_info = Some((*index, *count, hash));
            }
            Statement::WhileStatement(loop_) => {
                if loop_info.is_some() {
                    return None;
                }
                let PT::Op(op, terms) = protocol_term(model, &loop_.test)? else {
                    return None;
                };
                if op != "<" || terms.len() != 2 {
                    return None;
                }
                let (PT::Binding(index), PT::Binding(count)) = (&terms[0], &terms[1]) else {
                    return None;
                };
                if values.get(index) != Some(&SetupValue::Zero)
                    || values.get(count) != Some(&SetupValue::HashLength)
                {
                    return None;
                }
                let [step, update] = protocol_block(&loop_.body)? else {
                    return None;
                };
                let step = protocol_assignment(step)?;
                let hash = assignment_target_symbol(model, &step.left)?;
                if values.get(&hash) != Some(&SetupValue::Zero) || hash == *index {
                    return None;
                }
                let at = |index: PT| {
                    pc(
                        pm(pt("index", vec![array.clone(), index]), "charCodeAt"),
                        vec![],
                    )
                };
                let mix = |x: PT| {
                    pb(
                        "|",
                        pb("+", pb("-", pb("<<", ps(hash), pn(5.)), ps(hash)), x),
                        pn(0.),
                    )
                };
                if protocol_term(model, &step.right)
                    != Some(mix(pb(
                        "|",
                        pb("<<", at(ps(*index)), pn(16.)),
                        at(pb("+", ps(*index), pn(1.))),
                    )))
                {
                    return None;
                }
                let Statement::ExpressionStatement(update) = update else {
                    return None;
                };
                let Expression::AssignmentExpression(update) =
                    update.expression.without_parentheses()
                else {
                    return None;
                };
                if assignment_target_symbol(model, &update.left) != Some(*index) {
                    return None;
                }
                let increments = (update.operator.as_str() == "+=" && is_numeric(&update.right, 2.))
                    || (update.operator.as_str() == "="
                        && protocol_term(model, &update.right)
                            == Some(pb("+", ps(*index), pn(2.))));
                if !increments {
                    return None;
                }
                *assignments.entry(*index).or_default() += 1;
                *assignments.entry(hash).or_default() += 1;
                loop_info = Some((*index, *count, hash));
            }
            Statement::IfStatement(i) => {
                let term = protocol_term(model, &i.test)?;
                let typeof_function = |name: &str| pb("===", pt("typeof", vec![pg(name)]), PT::String("function".to_string()));
                let [yes] = protocol_block(&i.consequent)? else { return None; };
                let yes = protocol_assignment(yes)?;
                let b = assignment_target_symbol(model, &yes.left)?;
                if !declared.contains(&b) { return None; }
                if term == typeof_function("Uint8Array") && !values.contains_key(&b) {
                    let [no] = protocol_block(i.alternate.as_ref()?)? else { return None; };
                    let no = protocol_assignment(no)?;
                    if assignment_target_symbol(model, &no.left) != Some(b)
                        || protocol_term(model, &yes.right) != Some(pg("Uint8Array"))
                        || protocol_term(model, &no.right) != Some(PT::Null) { return None; }
                    values.insert(b, SetupValue::Uint8);
                    *assignments.entry(b).or_default() += 2;
                } else if term == ps(b) && values.get(&b) == Some(&SetupValue::Uint8)
                    && i.alternate.is_none() && protocol_term(model, &yes.right) == Some(typeof_function("Float64Array"))
                {
                    values.insert(b, SetupValue::Float64Guard);
                    *assignments.entry(b).or_default() += 1;
                } else if !values.contains_key(&b) {
                    let [no] = protocol_block(i.alternate.as_ref()?)? else { return None; };
                    let no = protocol_assignment(no)?;
                    if assignment_target_symbol(model, &no.left) != Some(b) { return None; }
                    let float_guard = values.iter().find_map(|(&capture, value)| {
                        (*value == SetupValue::Uint8
                            && term == pb("&&", ps(capture), typeof_function("Float64Array")))
                        .then_some(capture)
                    });
                    let nan_guard = values.iter().find_map(|(&capture, value)| {
                        (*value == SetupValue::Float64 && term == ps(capture)).then_some(capture)
                    });
                    if let Some(_capture) = float_guard {
                        if protocol_term(model, &yes.right)
                            != Some(pt("new", vec![pg("Float64Array"), pn(1.)]))
                            || protocol_term(model, &no.right) != Some(PT::Null)
                        { return None; }
                        values.insert(b, SetupValue::Float64);
                        *assignments.entry(b).or_default() += 2;
                    } else if nan_guard.is_some() {
                        if protocol_term(model, &yes.right) != Some(PT::Null)
                            || !protocol_nan(model, &no.right)
                        { return None; }
                        values.insert(b, SetupValue::Nan);
                        *assignments.entry(b).or_default() += 2;
                    } else { return None; }
                } else { return None; }
            }
            Statement::VariableDeclaration(d) => {
                for v in &d.declarations {
                    let b = declared_symbol(model, v)?;
                    if !declared.insert(b) {
                        return None;
                    }
                    if let Some(e) = &v.init {
                        let value = protocol_setup_value(
                            model, e, &values, payload, &length, children, loop_info,
                        )?;
                        if value == SetupValue::Key {
                            if key.replace(b).is_some() {
                                return None;
                            }
                        }
                        if values.insert(b, value).is_some() {
                            return None;
                        }
                    }
                }
            }
            Statement::ExpressionStatement(e) => {
                if let Some(a) = protocol_assignment(statement) {
                    let b = assignment_target_symbol(model, &a.left)?;
                    if !declared.contains(&b) || values.contains_key(&b) {
                        return None;
                    }
                    let value = protocol_setup_value(
                        model, &a.right, &values, payload, &length, children, loop_info,
                    )?;
                    if value == SetupValue::Key {
                        if key.replace(b).is_some() {
                            return None;
                        }
                    }
                    values.insert(b, value);
                    *assignments.entry(b).or_default() += 1;
                } else if path_is(
                    model,
                    &e.expression,
                    &["Object", "prototype", "hasOwnProperty"],
                ) && !own_read
                {
                    own_read = true;
                } else {
                    let c = call_of(&e.expression)?;
                    let args = call_arguments(c);
                    if c.arguments.len() != 1
                        || args.len() != 1
                        || symbol(model, &c.callee) != emitter.binding_symbol
                        || protocol_term(model, args[0]) != Some(pm(ps(data), "b"))
                        || key.is_none()
                        || loop_info.is_none()
                        || !own_read
                    {
                        return None;
                    }
                    called = true;
                }
            }
            _ => return None,
        }
    }
    if !called {
        return None;
    }
    for &b in &declared {
        if !model.scoping.symbol_redeclarations(b).is_empty()
            || model.binding_writes(b) != *assignments.get(&b).unwrap_or(&0)
        {
            return None;
        }
    }
    // Every wrapper capture used anywhere below an owned child must be live at
    // the real internal call. There is no traversal of external wrapper refs.
    for n in model.nodes.iter() {
        if n.span().start < container.span.start
            || n.span().end > container.span.end
            || owned(model, n.id(), container)
        {
            continue;
        }
        if let AstKind::IdentifierReference(i) = n.kind() {
            if let Some(b) = crate::analysis::identifier_symbol(i, model.scoping) {
                if declared.contains(&b) && !values.contains_key(&b) {
                    return None;
                }
            }
        }
    }
    let captures = values
        .clone()
        .into_iter()
        .filter_map(|(b, v)| match v {
            SetupValue::Payload => Some((b, PV::Payload)),
            SetupValue::Callable(n) => Some((b, PV::Function(n))),
            _ => None,
        })
        .collect();
    Some(WrapperSetup {
        captures,
        key: key?,
        kinds: values,
    })
}
fn protocol_setup_value(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    values: &BTreeMap<Symbol, SetupValue>,
    payload: Symbol,
    length: &PT,
    children: &[FunctionSite],
    loop_info: Option<(Symbol, Symbol, Symbol)>,
) -> Option<SetupValue> {
    if symbol(model, e) == Some(payload) {
        return Some(SetupValue::Payload);
    }
    if let Some(child) = children
        .iter()
        .find(|c| symbol(model, e) == c.binding_symbol)
    {
        return Some(SetupValue::Callable(child.span.start));
    }
    if let Some(b) = symbol(model, e) {
        if let Some(value) = values.get(&b) {
            if matches!(
                value,
                SetupValue::Uint8
                    | SetupValue::Float64
                    | SetupValue::Float64Guard
                    | SetupValue::Nan
                    | SetupValue::CharacterCode
            ) {
                return Some(value.clone());
            }
        }
    }
    let term = protocol_term(model, e);
    if term
        == Some(pc(
            pm(pm(pg("Array"), "call"), "bind"),
            vec![pm(pm(pg("String"), "prototype"), "charCodeAt")],
        ))
    {
        return Some(SetupValue::CharacterCode);
    }
    let typeof_function = |name: &str| {
        pb(
            "===",
            pt("typeof", vec![pg(name)]),
            PT::String("function".to_string()),
        )
    };
    if term
        == Some(pt(
            "?:",
            vec![typeof_function("Uint8Array"), pg("Uint8Array"), PT::Null],
        ))
    {
        return Some(SetupValue::Uint8);
    }
    for (&b, v) in values {
        if *v == SetupValue::Float64Guard && term == Some(pt("?:", vec![ps(b), pt("new", vec![pg("Float64Array"), pn(1.)]), PT::Null])) {
            return Some(SetupValue::Float64);
        }
        if *v == SetupValue::Uint8
            && term

                == Some(pt(
                    "?:",
                    vec![
                        pb("&&", ps(b), typeof_function("Float64Array")),
                        pt("new", vec![pg("Float64Array"), pn(1.)]),
                        PT::Null,
                    ],
                ))
        {
            return Some(SetupValue::Float64);
        }
        if *v == SetupValue::Float64 {
            if let Expression::ConditionalExpression(c) = e.without_parentheses() {
                if symbol(model, &c.test) == Some(b)
                    && matches!(
                        c.consequent.without_parentheses(),
                        Expression::NullLiteral(_)
                    )
                    && protocol_nan(model, &c.alternate)
                {
                    return Some(SetupValue::Nan);
                }
            }
        }
    }
    if term == Some(pb("<<", pb(">>", length.clone(), pn(1.)), pn(1.))) {
        return Some(SetupValue::HashLength);
    }
    if is_numeric(e, 0.) {
        return Some(SetupValue::Zero);
    }
    if let Some((_index, count, hash)) = loop_info {
        let PT::Op(_, length_args) = length else {
            return None;
        };
        let array = length_args.first()?.clone();
        let at = pc(
            pm(pt("index", vec![array, ps(count)]), "charCodeAt"),
            vec![],
        );
        let final_mix = pb(
            "|",
            pb(
                "+",
                pb("-", pb("<<", ps(hash), pn(5.)), ps(hash)),
                pb("<<", at, pn(16.)),
            ),
            pn(0.),
        );
        if term
            == Some(pt(
                "?:",
                vec![pb("!==", ps(count), length.clone()), final_mix, ps(hash)],
            ))
        {
            return Some(SetupValue::Key);
        }
    }
    None
}
fn closed_protocol(
    model: &Model<'_, '_>,
    container: FunctionSite,
    emitter: FunctionSite,
    stream: FunctionSite,
    writer: FunctionSite,
    number: FunctionSite,
    children: &[FunctionSite],
    proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Option<EmitterProof> {
    let AstKind::Function(f) = model.nodes.kind(emitter.node_id) else {
        return None;
    };
    if !plain_parameters(model, f, 0) {
        return None;
    }
    let nested = direct_children(model, emitter);
    let [count] = nested.as_slice() else {
        return None;
    };
    if role_at(proposals, count.span.start) != Some("countOwnPropertiesAlongPrototypeChain") {
        return None;
    }
    let setup = protocol_wrapper(model, container, emitter, children)?;
    let ctx = ProtocolContext {
        container,
        emitter,
        count: *count,
        stream,
        writer,
        number,
        proposals,
        captures: setup.captures.clone(),
    };
    let mut state = EmitterState {
        values: BTreeMap::new(),
        fields: 0,
        cursor: 0,
        header: false,
        paired: false,
        posted: false,
        count: false,
        stream: false,
        writer: false,
        word: false,
        direct_hardware_guard: false,
    };
    protocol_statements(
        model,
        &ctx,
        &mut state,
        &protocol_body(model, emitter)?.statements,
    )?;
    if !state.posted || !state.count || !state.stream || !state.writer || state.fields != 31 {
        return None;
    }
    protocol_stream_factory(model, stream, setup.key, proposals)?;
    protocol_writer_factory(model, writer, &ctx, &setup)
        .or_else(|| numeric_protocol::prove_writer(model, writer, &setup, number, proposals))
        .or_else(|| writer_role_proof(model, writer, &setup, number, proposals))?;
    protocol_numeric_operations(model, number, &setup, proposals)?;
    Some(EmitterProof {
        keystream_factory: stream,
        writer_factory: writer,
        number_writer: number,
    })
}
fn protocol_local<'a>(
    model: &Model<'_, 'a>,
    s: &'a Statement<'a>,
) -> Option<(Symbol, &'a Expression<'a>)> {
    if let Statement::VariableDeclaration(d) = s {
        let [v] = d.declarations.as_slice() else {
            return None;
        };
        Some((declared_symbol(model, v)?, v.init.as_ref()?))
    } else {
        let a = protocol_assignment(s)?;
        Some((assignment_target_symbol(model, &a.left)?, &a.right))
    }
}
fn protocol_scalar(model: &Model<'_, '_>, s: &Statement<'_>, b: Symbol, t: PT) -> bool {
    protocol_local(model, s)
        .is_some_and(|(actual, e)| actual == b && protocol_term(model, e) == Some(t))
}
fn protocol_return(model: &Model<'_, '_>, s: &Statement<'_>, t: PT) -> bool {
    matches!(s,Statement::ReturnStatement(r) if r.argument.as_ref().and_then(|e|protocol_term(model,e))==Some(t))
}
fn protocol_stream_factory(
    model: &Model<'_, '_>,
    factory: FunctionSite,
    key: Symbol,
    proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Option<()> {
    let AstKind::Function(f) = model.nodes.kind(factory.node_id) else {
        return None;
    };
    if !plain_parameters(model, f, 1) {
        return None;
    }
    let input = protocol_formal(model, factory, 0)?;
    let children = direct_children(model, factory);
    let [next] = children.as_slice() else {
        return None;
    };
    if role_at(proposals, next.span.start) != Some("nextKeystreamByte") {
        return None;
    }
    let body = protocol_body(model, factory)?;
    let mut decls = BTreeSet::new();
    let mut stores = vec![];
    let mut warmed = 0;
    let mut returned = false;
    for s in &body.statements {
        if returned {
            return None;
        }
        match s {
            Statement::VariableDeclaration(d)
                if d.declarations.iter().all(|v| v.init.is_none()) =>
            {
                if !stores.is_empty() || warmed != 0 {
                    return None;
                }
                for v in &d.declarations {
                    if !decls.insert(declared_symbol(model, v)?) {
                        return None;
                    }
                }
            }
            Statement::FunctionDeclaration(f) if f.span == next.span => {}
            Statement::ReturnStatement(_)
                if warmed == 4 && protocol_return(model, s, ps(next.binding_symbol?)) =>
            {
                returned = true;
            }
            Statement::ExpressionStatement(e) if call_of(&e.expression).is_some() => {
                let c = call_of(&e.expression)?;
                if !c.arguments.is_empty()
                    || symbol(model, &c.callee) != next.binding_symbol
                    || stores.len() != 3
                    || warmed >= 4
                {
                    return None;
                }
                warmed += 1;
            }
            _ => {
                if warmed != 0 {
                    return None;
                }
                let (b, e) = protocol_local(model, s)?;
                if !decls.contains(&b) || stores.iter().any(|(x, _)| *x == b) {
                    return None;
                }
                stores.push((b, e));
            }
        }
    }
    if !returned || decls.len() != 4 {
        return None;
    }
    let a = stores
        .iter()
        .find_map(|(b, e)| (symbol(model, e) == Some(key)).then_some(*b))?;
    let b = stores
        .iter()
        .find_map(|(b, e)| (symbol(model, e) == Some(input)).then_some(*b))?;
    let counter = stores
        .iter()
        .find_map(|(b, e)| (numeric(e) == Some(-1.)).then_some(*b))?;
    let sum = *decls.iter().find(|&&v| v != a && v != b && v != counter)?;
    if [a, b, counter, sum]
        .into_iter()
        .collect::<BTreeSet<_>>()
        .len()
        != 4
    {
        return None;
    }
    let next_body = protocol_body(model, *next)?;
    let [first, advance, Statement::IfStatement(zero), Statement::IfStatement(one), Statement::IfStatement(two), Statement::IfStatement(three)] =
        next_body.statements.as_slice()
    else {
        return None;
    };
    let (snapshot, e) = protocol_local(model, first)?;
    if symbol(model, e) != Some(counter) {
        return None;
    }
    let phase = pb("%", pb("+", ps(snapshot), pn(1.)), pn(4.));
    if !protocol_scalar(model, advance, counter, phase.clone()) {
        return None;
    }
    for (i, branch) in [zero, one, two, three].iter().enumerate() {
        if protocol_term(model, &branch.test) != Some(pb("===", phase.clone(), pn(i as f64))) {
            return None;
        }
        if i < 3 && branch.alternate.is_some() {
            return None;
        }
    }
    let [sum_store, snapshot_store, xor_store, mix_store, rotate_store, ret] =
        protocol_block(&zero.consequent)?
    else {
        return None;
    };
    let (saved, e) = protocol_local(model, snapshot_store)?;
    if symbol(model, e) != Some(b) {
        return None;
    }
    let rotated = pb("|", pb("<<", ps(a), pn(26.)), pb(">>>", ps(a), pn(6.)));
    if !protocol_scalar(model, sum_store, sum, pb("+", ps(a), ps(b)))
        || !protocol_scalar(model, xor_store, b, pb("^", ps(saved), ps(a)))
        || !protocol_scalar(
            model,
            mix_store,
            a,
            pb(
                "^",
                pb("^", pb("^", ps(saved), ps(a)), rotated),
                pb("<<", ps(b), pn(9.)),
            ),
        )
        || !protocol_scalar(
            model,
            rotate_store,
            b,
            pb("|", pb("<<", ps(b), pn(13.)), pb(">>>", ps(b), pn(19.))),
        )
        || !protocol_return(model, ret, pb("&", ps(sum), pn(255.)))
    {
        return None;
    }
    for (branch, shift) in [(one, 8.), (two, 16.), (three, 24.)] {
        let [ret] = protocol_block(&branch.consequent)? else {
            return None;
        };
        if !protocol_return(
            model,
            ret,
            pb(
                "&",
                pb(if shift == 24. { ">>>" } else { ">>" }, ps(sum), pn(shift)),
                pn(255.),
            ),
        ) {
            return None;
        }
    }
    let [ret] = protocol_block(three.alternate.as_ref()?)? else {
        return None;
    };
    if !protocol_return(model, ret, pt("void", vec![pn(0.)])) {
        return None;
    }
    Some(())
}
#[derive(Clone, Debug, PartialEq)]
enum ProtocolInstruction {
    Empty(Symbol),
    Store(PT, PT),
    If(PT, Vec<Self>, Vec<Self>),
    While(PT, Vec<Self>),
    Eval(PT),
    Return(PT),
}
use ProtocolInstruction as PI;
fn protocol_instructions(model: &Model<'_, '_>, statements: &[Statement<'_>]) -> Option<Vec<PI>> {
    let mut result = vec![];
    for s in statements {
        match s {
            Statement::VariableDeclaration(d) => {
                for v in &d.declarations {
                    let b = declared_symbol(model, v)?;
                    result.push(if let Some(e) = &v.init {
                        PI::Store(ps(b), protocol_term(model, e)?)
                    } else {
                        PI::Empty(b)
                    });
                }
            }
            Statement::ExpressionStatement(e) => {
                if let Some(a) = protocol_assignment(s) {
                    let lhs = match &a.left {
                        AssignmentTarget::AssignmentTargetIdentifier(i) => {
                            ps(crate::analysis::identifier_symbol(i, model.scoping)?)
                        }
                        AssignmentTarget::ComputedMemberExpression(m) if !m.optional => pt(
                            "index",
                            vec![
                                protocol_term(model, &m.object)?,
                                protocol_term(model, &m.expression)?,
                            ],
                        ),
                        _ => return None,
                    };
                    result.push(PI::Store(lhs, protocol_term(model, &a.right)?));
                } else {
                    result.push(PI::Eval(protocol_term(model, &e.expression)?));
                }
            }
            Statement::IfStatement(i) => result.push(PI::If(
                protocol_term(model, &i.test)?,
                protocol_instruction_arm(model, &i.consequent)?,
                if let Some(a) = &i.alternate {
                    protocol_instruction_arm(model, a)?
                } else {
                    vec![]
                },
            )),
            Statement::WhileStatement(w) => result.push(PI::While(
                protocol_term(model, &w.test)?,
                protocol_instruction_arm(model, &w.body)?,
            )),
            Statement::ReturnStatement(r) => {
                result.push(PI::Return(protocol_term(model, r.argument.as_ref()?)?))
            }
            _ => return None,
        }
    }
    Some(result)
}
fn protocol_instruction_arm(model: &Model<'_, '_>, s: &Statement<'_>) -> Option<Vec<PI>> {
    if let Statement::BlockStatement(b) = s {
        protocol_instructions(model, &b.body)
    } else {
        protocol_instructions(model, std::slice::from_ref(s))
    }
}
fn pi_binding(p: &PI) -> Option<Symbol> {
    match p {
        PI::Store(PT::Binding(b), _) | PI::Empty(b) => Some(*b),
        _ => None,
    }
}
fn pi_set(b: Symbol, t: PT) -> PI {
    PI::Store(ps(b), t)
}
fn pi_index(a: Symbol, i: PT) -> PT {
    pt("index", vec![ps(a), i])
}
fn pi_byte(buffer: Symbol, cursor: Symbol, value: PT, mask: Symbol) -> PI {
    let mask = pc(ps(mask), vec![]);
    let payload = if matches!(value, PT::Number(_)) {
        pb("^", mask, value)
    } else {
        pb("^", value, mask)
    };
    PI::Store(pi_index(buffer, pt("pre++", vec![ps(cursor)])), payload)
}
fn protocol_writer_factory(
    model: &Model<'_, '_>,
    factory: FunctionSite,
    ctx: &ProtocolContext<'_>,
    setup: &WrapperSetup,
) -> Option<()> {
    let AstKind::Function(f) = model.nodes.kind(factory.node_id) else {
        return None;
    };
    if !plain_parameters(model, f, 1) {
        return None;
    }
    let input = protocol_formal(model, factory, 0)?;
    let nested = direct_children(model, factory);
    let [writer] = nested.as_slice() else {
        return None;
    };
    if role_at(ctx.proposals, writer.span.start) != Some("writeInternedString") {
        return None;
    }
    let mut empty = BTreeSet::new();
    let mut slots = BTreeSet::new();
    let mut mask = None;
    let mut cache = None;
    let mut counter = None;
    let mut returned = false;
    for s in &protocol_body(model, factory)?.statements {
        if returned {
            return None;
        }
        if let Statement::VariableDeclaration(d) = s {
            for v in &d.declarations {
                if !slots.insert(declared_symbol(model, v)?) {
                    return None;
                }
            }
        }
        match s {
            Statement::FunctionDeclaration(f) if f.span == writer.span => {}
            Statement::VariableDeclaration(d)
                if d.declarations.iter().all(|v| v.init.is_none()) =>
            {
                for v in &d.declarations {
                    if !empty.insert(declared_symbol(model, v)?) {
                        return None;
                    }
                }
            }
            Statement::ReturnStatement(_)
                if protocol_return(model, s, ps(writer.binding_symbol?)) =>
            {
                returned = true;
            }
            _ => {
                let (b, e) = protocol_local(model, s)?;
                if symbol(model, e) == Some(input) && mask.is_none() {
                    mask = Some(b);
                } else if protocol_term(model, e)
                    == Some(pc(pm(pg("Object"), "create"), vec![PT::Null]))
                    && cache.is_none()
                    && slots.contains(&b)
                {
                    cache = Some(b);
                } else if is_numeric(e, 0.) && counter.is_none() && slots.contains(&b) {
                    counter = Some(b);
                } else {
                    return None;
                }
            }
        }
    }
    if !returned || slots.len() != 3 {
        return None;
    }
    let mask = mask?;
    let cache = cache?;
    let counter = counter?;
    let text = protocol_formal(model, *writer, 0)?;
    let buffer = protocol_formal(model, *writer, 1)?;
    let position = protocol_formal(model, *writer, 2)?;
    let actual = protocol_instructions(model, &protocol_body(model, *writer)?.statements)?;
    let [PI::Empty(result), PI::If(test, hit, miss), PI::Return(ret)] = actual.as_slice() else {
        return None;
    };
    let result = *result;
    if *test != pb("!=", pi_index(cache, ps(text)), PT::Null) || *ret != ps(result) {
        return None;
    }
    let [len, idx, cur, PI::While(_, loop_body), advance, terminal, cache_alias, old_counter, counter_inc, cache_store] =
        miss.as_slice()
    else {
        return None;
    };
    let length = pi_binding(len)?;
    let index = pi_binding(idx)?;
    let cursor = pi_binding(cur)?;
    let alias = pi_binding(cache_alias)?;
    let old = pi_binding(old_counter)?;
    let code = pi_binding(loop_body.first()?)?;
    let (split, if_index) = if matches!(loop_body.get(1), Some(PI::Empty(_))) {
        (true, 2)
    } else {
        (false, 1)
    };
    let PI::If(_, zero, nonzero) = loop_body.get(if_index)? else {
        return None;
    };
    let second = if split {
        let [PI::If(_, branches, _), _] = nonzero.as_slice() else {
            return None;
        };
        branches
    } else {
        let [PI::If(_, branches, _)] = nonzero.as_slice() else {
            return None;
        };
        branches
    };
    let [PI::If(_, _, wide)] = second.as_slice() else {
        return None;
    };
    let [next_decl, _] = wide.as_slice() else {
        return None;
    };
    let next = pi_binding(next_decl)?;
    // Bind the actual captured producer edges to their proved wrapper setup.
    let PI::Store(_, PT::Op(call, code_args)) = loop_body.first()? else {
        return None;
    };
    if call != "call" || code_args.len() != 3 {
        return None;
    }
    let PT::Binding(code_reader) = code_args[0] else {
        return None;
    };
    if setup.kinds.get(&code_reader) != Some(&SetupValue::CharacterCode) {
        return None;
    }
    let Some(PI::Store(_, PT::Op(call, num_args))) = hit.last() else {
        return None;
    };
    if call != "call" || num_args.len() != 5 {
        return None;
    }
    let PT::Binding(number) = num_args[0] else {
        return None;
    };
    if ctx.captures.get(&number) != Some(&PV::Function(ctx.number.span.start)) {
        return None;
    }
    let mask_call = pc(ps(mask), vec![]);
    let n = |op: &str, a: PT, b: f64| pb(op, a, pn(b));
    let continuation = |value: PT| n("|", n("&", value, 63.), 128.);
    let pair_value = pb(
        "+",
        pb("+", pn(65536.), n("<<", n("-", ps(code), 55296.), 10.)),
        n("-", ps(next), 56320.),
    );
    let check_surrogate = pb(
        "&&",
        pb(
            "&&",
            pb("&&", n(">=", ps(code), 55296.), n("<=", ps(code), 57343.)),
            n("<", ps(code), 56320.),
        ),
        pb("<", n("+", ps(index), 1.), ps(length)),
    );
    let next_value = pt(
        "?:",
        vec![
            check_surrogate,
            pc(
                ps(code_reader),
                vec![pi_index(text, n("+", ps(index), 1.)), pn(0.)],
            ),
            PT::Null,
        ],
    );
    let wide_plan = vec![
        pi_set(next, next_value),
        PI::If(
            pb(
                "||",
                pb("||", pb("==", ps(next), PT::Null), n("<", ps(next), 56320.)),
                n(">", ps(next), 57343.),
            ),
            vec![
                pi_byte(buffer, cursor, n("|", n(">>", ps(code), 12.), 224.), mask),
                pi_byte(
                    buffer,
                    cursor,
                    n("|", n("&", n(">>", ps(code), 6.), 63.), 128.),
                    mask,
                ),
                pi_set(code, continuation(ps(code))),
            ],
            vec![
                PI::Eval(pt("pre++", vec![ps(index)])),
                pi_byte(
                    buffer,
                    cursor,
                    n("|", n(">>", pair_value.clone(), 18.), 240.),
                    mask,
                ),
                pi_byte(
                    buffer,
                    cursor,
                    continuation(n(">>", pair_value.clone(), 12.)),
                    mask,
                ),
                pi_byte(
                    buffer,
                    cursor,
                    continuation(n(">>", pair_value.clone(), 6.)),
                    mask,
                ),
                pi_set(code, continuation(pair_value)),
            ],
        ),
    ];
    let nonzero_plan = PI::If(
        pt("!", vec![n("<", ps(code), 128.)]),
        vec![PI::If(
            n("<", ps(code), 2048.),
            vec![
                pi_byte(buffer, cursor, n("|", n(">>", ps(code), 6.), 192.), mask),
                pi_set(code, continuation(ps(code))),
            ],
            wide_plan,
        )],
        vec![],
    );
    let mut loop_plan = vec![pi_set(
        code,
        pc(ps(code_reader), vec![pi_index(text, ps(index)), pn(0.)]),
    )];
    if split {
        let Some(PI::Empty(byte_mask)) = loop_body.get(1) else {
            return None;
        };
        let byte_mask = *byte_mask;
        loop_plan.push(PI::Empty(byte_mask));
        loop_plan.push(PI::If(
            n("===", ps(code), 0.),
            vec![
                pi_byte(buffer, cursor, pn(192.), mask),
                pi_set(code, mask_call.clone()),
                pi_set(byte_mask, pn(128.)),
            ],
            vec![nonzero_plan, pi_set(byte_mask, mask_call.clone())],
        ));
        loop_plan.push(PI::Store(
            pi_index(buffer, pt("pre++", vec![ps(cursor)])),
            pb("^", ps(code), ps(byte_mask)),
        ));
    } else {
        let PI::If(test, yes, no) = nonzero_plan else {
            return None;
        };
        loop_plan.push(PI::If(
            n("===", ps(code), 0.),
            vec![
                pi_byte(buffer, cursor, pn(192.), mask),
                pi_set(code, pn(128.)),
            ],
            vec![PI::If(test, yes, no)],
        ));
        loop_plan.push(pi_byte(buffer, cursor, ps(code), mask));
    }
    loop_plan.push(PI::Eval(pt("pre++", vec![ps(index)])));
    let miss_plan = vec![
        pi_set(length, pm(ps(text), "length")),
        pi_set(index, pn(0.)),
        pi_set(cursor, ps(position)),
        PI::While(pb("<", ps(index), ps(length)), loop_plan),
        pi_set(result, n("+", pc(pg("Number"), vec![ps(cursor)]), 1.)),
        PI::Store(pi_index(buffer, ps(result)), mask_call.clone()),
        pi_set(alias, ps(cache)),
        pi_set(old, pc(pg("Number"), vec![ps(counter)])),
        pi_set(counter, n("+", ps(old), 1.)),
        PI::Store(pi_index(alias, ps(text)), ps(old)),
    ];
    if *miss != miss_plan {
        return None;
    }
    let last = pi_set(
        result,
        pc(
            ps(number),
            vec![ps(mask), pi_index(cache, ps(text)), ps(buffer), ps(result)],
        ),
    );
    let hit_prefix = pi_set(result, ps(position));
    let direct = vec![
        hit_prefix.clone(),
        pi_byte(buffer, result, pn(129.), mask),
        last.clone(),
    ];
    let advanced = vec![
        hit_prefix,
        pi_set(result, n("+", pc(pg("Number"), vec![ps(position)]), 1.)),
        PI::Store(pi_index(buffer, ps(result)), pb("^", mask_call, pn(129.))),
        last,
    ];
    if *hit != direct && *hit != advanced {
        return None;
    }
    let _ = (zero, advance, terminal, counter_inc, cache_store);
    Some(())
}

#[path="worker_environment_numeric.rs"]
mod numeric_protocol;

/// True only for the accepted sharing evaluator pair of one role. This is not a
/// member/presence check: the shared evaluator already proved the complete
/// helper body (scope, arithmetic, stores and returns).
fn claimed_role(
    proposals: &BTreeMap<u32, Vec<Proposal>>,
    start: u32,
    role: &str,
    evaluator: &str,
) -> bool {
    proposals.get(&start).is_some_and(|claims| {
        claims
            .iter()
            .any(|claim| claim.name == role && claim.evaluator == evaluator)
    })
}

fn arm_store_matches(
    model: &Model<'_, '_>,
    site: FunctionSite,
    span: Span,
    zero: bool,
) -> bool {
    for node in model.nodes.iter() {
        if !owned(model, node.id(), site)
            || node.span().start < span.start
            || node.span().end > span.end
        {
            continue;
        }
        let value = match node.kind() {
            AstKind::VariableDeclarator(declarator) => declarator.init.as_ref(),
            AstKind::AssignmentExpression(assignment) => Some(&assignment.right),
            _ => None,
        };
        if let Some(value) = value {
            let value = value.without_parentheses();
            if zero && is_zero(value) {
                return true;
            }
            if !zero {
                if let Expression::BinaryExpression(binary) = value {
                    if binary.operator.as_str() == "-" {
                        return true;
                    }
                }
            }
        }
    }
    false
}

fn is_carry_test_expression(model: &Model<'_, '_>, expression: &Expression<'_>) -> bool {
    let Expression::BinaryExpression(compare) = expression.without_parentheses() else {
        return false;
    };
    if compare.operator.as_str() != ">=" || !is_numeric(&compare.right, 1.) {
        return false;
    }
    let Expression::BinaryExpression(division) = compare.left.without_parentheses() else {
        return false;
    };
    if division.operator.as_str() != "/" {
        return false;
    }
    matches!(
        division.left.without_parentheses(),
        Expression::BinaryExpression(sub) if sub.operator.as_str() == "-"
    )
}

fn symbol_has_subtraction_store(
    model: &Model<'_, '_>,
    site: FunctionSite,
    binding: Symbol,
) -> bool {
    for node in model.nodes.iter() {
        if !owned(model, node.id(), site) {
            continue;
        }
        let value = match node.kind() {
            AstKind::VariableDeclarator(declarator)
                if declarator.init.is_some() && declared_symbol(model, declarator) == Some(binding) =>
            {
                declarator.init.as_ref()
            }
            AstKind::AssignmentExpression(assignment)
                if assignment.operator.as_str() == "="
                    && assignment_target_symbol(model, &assignment.left) == Some(binding) =>
            {
                Some(&assignment.right)
            }
            _ => None,
        };
        if let Some(Expression::BinaryExpression(sub)) =
            value.map(|value| value.without_parentheses())
        {
            if sub.operator.as_str() == "-" {
                return true;
            }
        }
    }
    false
}

fn has_fraction_select(model: &Model<'_, '_>, site: FunctionSite) -> bool {
    for node in model.nodes.iter() {
        if !owned(model, node.id(), site) {
            continue;
        }
        match node.kind() {
            AstKind::ConditionalExpression(conditional)
                if matches!(
                    conditional.test.without_parentheses(),
                    Expression::Identifier(_)
                ) =>
            {
                let (yes, no) = (
                    conditional.consequent.without_parentheses(),
                    conditional.alternate.without_parentheses(),
                );
                let yes_zero = is_zero(yes);
                let no_zero = is_zero(no);
                let yes_sub = matches!(yes, Expression::BinaryExpression(sub) if sub.operator.as_str() == "-");
                let no_sub = matches!(no, Expression::BinaryExpression(sub) if sub.operator.as_str() == "-");
                if (yes_zero && no_sub) || (no_zero && yes_sub) {
                    return true;
                }
                if yes_zero {
                    if let Some(binding) = symbol(model, no) {
                        if symbol_has_subtraction_store(model, site, binding) {
                            return true;
                        }
                    }
                }
            }
            AstKind::IfStatement(if_) => {
                let Some(alternate) = if_.alternate.as_ref() else {
                    continue;
                };
                if !is_carry_test_expression(model, &if_.test) {
                    continue;
                }
                let yes = if_.consequent.span();
                let no = alternate.span();
                if (arm_store_matches(model, site, yes, true)
                    && arm_store_matches(model, site, no, false))
                    || (arm_store_matches(model, site, yes, false)
                        && arm_store_matches(model, site, no, true))
                {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

fn carry_conditional(model: &Model<'_, '_>, conditional: &ConditionalExpression<'_>) -> bool {
    let Expression::BinaryExpression(yes) = conditional.consequent.without_parentheses() else {
        return false;
    };
    if yes.operator.as_str() != "+" {
        return false;
    }
    let same = symbol(model, &yes.left);
    if is_numeric(&yes.right, 1.) && symbol(model, &conditional.alternate) == same {
        return true;
    }
    if is_numeric(&yes.right, 1024.) {
        if let Expression::BinaryExpression(no) = conditional.alternate.without_parentheses() {
            if no.operator.as_str() == "+"
                && is_numeric(&no.right, 1023.)
                && symbol(model, &no.left) == same
            {
                return true;
            }
        }
    }
    false
}

fn has_exponent_carry(model: &Model<'_, '_>, site: FunctionSite) -> bool {
    let mut exponents = BTreeSet::new();
    for node in model.nodes.iter() {
        if !owned(model, node.id(), site) {
            continue;
        }
        let target = match node.kind() {
            AstKind::VariableDeclarator(declarator)
                if declarator.init.as_ref().is_some_and(|init| is_numeric(init, 2047.)) =>
            {
                declared_symbol(model, declarator)
            }
            AstKind::AssignmentExpression(assignment)
                if assignment.operator.as_str() == "=" && is_numeric(&assignment.right, 2047.) =>
            {
                assignment_target_symbol(model, &assignment.left)
            }
            _ => None,
        };
        if let Some(symbol) = target {
            exponents.insert(symbol);
        }
    }
    if exponents.is_empty() {
        return false;
    }
    for node in model.nodes.iter() {
        if !owned(model, node.id(), site) {
            continue;
        }
        let (target, value) = match node.kind() {
            AstKind::VariableDeclarator(declarator)
                if declarator.init.is_some() =>
            {
                (declared_symbol(model, declarator), declarator.init.as_ref())
            }
            AstKind::AssignmentExpression(assignment) if assignment.operator.as_str() == "=" => {
                (assignment_target_symbol(model, &assignment.left), Some(&assignment.right))
            }
            _ => (None, None),
        };
        let Some(target) = target else {
            continue;
        };
        if !exponents.contains(&target) {
            continue;
        }
        let Some(value) = value else {
            continue;
        };
        if let Expression::ConditionalExpression(conditional) = value.without_parentheses() {
            if carry_conditional(model, conditional) {
                return true;
            }
        }
        if let Expression::BinaryExpression(binary) = value.without_parentheses() {
            if binary.operator.as_str() == "+" && is_numeric(&binary.right, 1023.) {
                if let Expression::ConditionalExpression(conditional) =
                    binary.left.without_parentheses()
                {
                    if carry_conditional(model, conditional) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

fn has_carry_test(model: &Model<'_, '_>, site: FunctionSite) -> bool {
    let mut numerators = Vec::new();
    for node in model.nodes.iter() {
        if !owned(model, node.id(), site) {
            continue;
        }
        let AstKind::BinaryExpression(compare) = node.kind() else {
            continue;
        };
        if compare.operator.as_str() != ">=" || !is_numeric(&compare.right, 1.) {
            continue;
        }
        let Expression::BinaryExpression(division) = compare.left.without_parentheses() else {
            continue;
        };
        if division.operator.as_str() != "/" {
            continue;
        }
        let Some(denominator) = symbol(model, &division.right) else {
            continue;
        };
        if let Expression::BinaryExpression(sub) = division.left.without_parentheses() {
            if sub.operator.as_str() == "-" && symbol(model, &sub.right) == Some(denominator) {
                return true;
            }
        }
        if let Some(numerator) = symbol(model, &division.left) {
            numerators.push((numerator, denominator));
        }
    }
    for (numerator, denominator) in numerators {
        for node in model.nodes.iter() {
            if !owned(model, node.id(), site) {
                continue;
            }
            let value = match node.kind() {
                AstKind::VariableDeclarator(declarator)
                    if declarator.init.is_some()
                        && declared_symbol(model, declarator) == Some(numerator) =>
                {
                    declarator.init.as_ref()
                }
                AstKind::AssignmentExpression(assignment)
                    if assignment.operator.as_str() == "="
                        && assignment_target_symbol(model, &assignment.left) == Some(numerator) =>
                {
                    Some(&assignment.right)
                }
                _ => None,
            };
            if let Some(Expression::BinaryExpression(sub)) =
                value.map(|value| value.without_parentheses())
            {
                if sub.operator.as_str() == "-" && symbol(model, &sub.right) == Some(denominator) {
                    return true;
                }
            }
        }
    }
    false
}

fn loop_has_bit_pack(model: &Model<'_, '_>, site: FunctionSite, body: Span) -> bool {
    let (mut mask, mut half) = (false, false);
    for node in model.nodes.iter() {
        if !owned(model, node.id(), site)
            || node.span().start < body.start
            || node.span().end > body.end
        {
            continue;
        }
        match node.kind() {
            AstKind::BinaryExpression(binary)
                if binary.operator.as_str() == "&" && is_numeric(&binary.right, 1.) =>
            {
                mask = true;
            }
            AstKind::CallExpression(call) => {
                let Expression::StaticMemberExpression(member) = call.callee.without_parentheses()
                else {
                    continue;
                };
                if member.property.name != "floor" || !path_is(model, &member.object, &["Math"]) {
                    continue;
                }
                let args = call_arguments(call);
                if let Some(Expression::BinaryExpression(division)) =
                    args.first().map(|arg| arg.without_parentheses())
                {
                    if division.operator.as_str() == "/" && is_numeric(&division.right, 2.) {
                        half = true;
                    }
                }
            }
            _ => {}
        }
    }
    mask && half
}

fn is_eight_loop_test(model: &Model<'_, '_>, test: &Expression<'_>) -> bool {
    matches!(
        test.without_parentheses(),
        Expression::BinaryExpression(binary)
            if binary.operator.as_str() == "<" && numeric(&binary.right) == Some(8.)
    )
}

/// The shared `writeNumber` evaluator proves the complete numeric encoder. The
/// bundle still proves the exact operation fragments and port bindings that tie
/// that accepted body to this wrapper. This is a closed, compositional grammar:
/// every numeric literal comes from the accepted fragment set, every callee is a
/// captured mask/typed-array port or a known `Math`/`Number`/array operation,
/// the varint thresholds/shifts/masks/sentinel, IEEE-754 constants and `+1023`
/// bias are present exactly, and the declared typed-array ports are consumed.
fn numeric_semantics_closed(
    model: &Model<'_, '_>,
    site: FunctionSite,
    setup: &WrapperSetup,
) -> Option<()> {
    const ALLOWED: &[f64] = &[
        -1074., -1022., 0., 0.5, 1., 2., 3., 4., 5., 6., 7., 8., 11., 12., 19., 26.,
        31., 32., 51., 52., 56., 64., 127., 128., 1022., 1023., 1024., 1074., 2046.,
        2047., 4096., 524288., 67108864.,
    ];
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return None;
    };
    if !plain_parameters(model, function, 4) {
        return None;
    }
    let mut thresholds = BTreeSet::new();
    let mut shifts = BTreeSet::new();
    let mut powers = BTreeSet::new();
    let mut mod2 = 0u32;
    let (mut shl7, mut bit_loops) = (false, 0u32);
    let (mut lt_half, mut gt_half) = (0u32, 0u32);
    let (mut xor128, mut min1023) = (false, false);
    let (mut gt2046, mut eq2047, mut bias1023) = (false, false, false);
    let (mut loop8, mut sign64) = (false, false);
    let mut loop8_body = None;
    let mut abs_value = false;
    let mut abs_direct = false;
    let mut abs_symbol = None;
    let (mut fraction_select, mut exponent_carry, mut carry_test) = (false, false, false);
    let mut scratch_write = false;
    let mut scratch_buffer = false;
    let mut byte_constructor = false;
    let mut nan_data = false;
    for node in model.nodes.iter() {
        if !owned(model, node.id(), site) {
            continue;
        }
        match node.kind() {
            AstKind::NumericLiteral(literal) => {
                if !ALLOWED.contains(&literal.value) {
                    return None;
                }
            }
            AstKind::UnaryExpression(unary) if unary.operator.as_str() == "-" => {
                if let Some(value) = numeric(&unary.argument) {
                    if !ALLOWED.contains(&(-value)) {
                        return None;
                    }
                }
            }
            AstKind::BinaryExpression(binary) => {
                let right = numeric(&binary.right);
                match binary.operator.as_str() {
                    "<" => {
                        if let Some(value) = right {
                            if matches!(value as i64, 32 | 4096 | 524288 | 67108864) {
                                thresholds.insert(value as i64);
                            }
                            if value == 0.5 {
                                lt_half += 1;
                            }
                        }
                    }
                    ">" => {
                        if right == Some(0.5) {
                            gt_half += 1;
                        }
                        if right == Some(2046.) {
                            gt2046 = true;
                        }
                    }
                    ">>>" => {
                        if let Some(value) = right {
                            if matches!(value as i64, 12 | 19 | 26) {
                                shifts.insert(value as i64);
                            }
                        }
                    }
                    ">>" => {
                        if right == Some(5.) {
                            shifts.insert(5);
                        }
                    }
                    "%" if right == Some(2.) => mod2 += 1,
                    "<<" if right == Some(7.) => shl7 = true,
                    "^" if right == Some(128.) => xor128 = true,
                    "+" if right == Some(1023.) => bias1023 = true,
                    _ => {}
                }
            }
            AstKind::AssignmentExpression(assignment) if assignment.operator.as_str() == "=" => {
                let right = numeric(&assignment.right);
                if right == Some(64.) {
                    sign64 = true;
                }
                if right == Some(2047.) {
                    eq2047 = true;
                }
                if let AssignmentTarget::ComputedMemberExpression(member) = &assignment.left {
                    if symbol(model, &member.object)
                        .is_some_and(|binding| setup.kinds.get(&binding) == Some(&SetupValue::Float64))
                        && is_zero(&member.expression)
                    {
                        scratch_write = true;
                    }
                }
            }
            AstKind::AssignmentExpression(assignment) if assignment.operator.as_str() == "+=" => {
                if numeric(&assignment.right) == Some(1023.) {
                    bias1023 = true;
                }
            }
            AstKind::VariableDeclarator(declarator) => {
                if let Some(init) = &declarator.init {
                    match numeric(init) {
                        Some(64.) => sign64 = true,
                        Some(2047.) => eq2047 = true,
                        _ => {}
                    }
                }
            }
            AstKind::ForStatement(loop_) => {
                if loop_
                    .test
                    .as_ref()
                    .is_some_and(|test| is_eight_loop_test(model, test))
                {
                    loop8 = true;
                    loop8_body = Some(loop_.body.span());
                }
                if loop_has_bit_pack(model, site, loop_.body.span()) {
                    bit_loops += 1;
                }
            }
            AstKind::WhileStatement(loop_) => {
                if is_eight_loop_test(model, &loop_.test) {
                    loop8 = true;
                    loop8_body = Some(loop_.body.span());
                }
                if loop_has_bit_pack(model, site, loop_.body.span()) {
                    bit_loops += 1;
                }
            }
            AstKind::StaticMemberExpression(member) if member.property.name == "buffer" => {
                if symbol(model, &member.object)
                    .is_some_and(|binding| setup.kinds.get(&binding) == Some(&SetupValue::Float64))
                {
                    scratch_buffer = true;
                }
            }
            AstKind::NewExpression(new) => {
                let Some(binding) = symbol(model, &new.callee) else {
                    return None;
                };
                if setup.kinds.get(&binding) != Some(&SetupValue::Uint8) {
                    return None;
                }
                byte_constructor = true;
            }
            AstKind::IdentifierReference(identifier) => {
                if crate::analysis::identifier_symbol(identifier, model.scoping)
                    .is_some_and(|binding| setup.kinds.get(&binding) == Some(&SetupValue::Nan))
                {
                    nan_data = true;
                }
            }
            AstKind::CallExpression(call) => {
                let callee = call.callee.without_parentheses();
                match callee {
                    Expression::Identifier(identifier) => {
                        if crate::analysis::identifier_symbol(identifier, model.scoping).is_none()
                            && identifier.name.as_str() != "Number"
                        {
                            return None;
                        }
                    }
                    Expression::StaticMemberExpression(member) if !member.optional => {
                        if !matches!(
                            member.property.name.as_str(),
                            "abs" | "min" | "floor" | "log" | "pow" | "push" | "unshift" | "call"
                        ) {
                            return None;
                        }
                        let args = call_arguments(call);
                        if path_is(model, &member.object, &["Math"]) {
                            if crate::analysis::expression_symbol(&member.object, model.scoping)
                                .is_some()
                            {
                                return None;
                            }
                            match member.property.name.as_str() {
                                "pow" if args.len() == 2 && numeric(args[0]) == Some(2.) => {
                                    if let Some(exponent) = numeric(args[1]) {
                                        powers.insert(exponent as i64);
                                    }
                                }
                                "min" if args.len() == 2 && numeric(args[1]) == Some(1023.) => {
                                    min1023 = true;
                                }
                                "abs" if args.len() == 1 => {
                                    if numeric_value_alias(model, site, args[0]) {
                                        abs_value = true;
                                        let direct = match model.nodes.parent_kind(node.id()) {
                                            AstKind::VariableDeclarator(declarator) => declarator
                                                .init
                                                .as_ref()
                                                .is_some_and(|init| init.span() == call.span()),
                                            AstKind::AssignmentExpression(assignment) => {
                                                assignment.right.span() == call.span()
                                            }
                                            _ => false,
                                        };
                                        if direct {
                                            let target = match model.nodes.parent_kind(node.id()) {
                                                AstKind::VariableDeclarator(declarator) => {
                                                    declared_symbol(model, declarator)
                                                }
                                                AstKind::AssignmentExpression(assignment) => {
                                                    assignment_target_symbol(model, &assignment.left)
                                                }
                                                _ => None,
                                            };
                                            if abs_symbol.is_none() {
                                                abs_symbol = Some(target);
                                            }
                                            abs_direct = true;
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    _ => return None,
                }
            }
            _ => {}
        }
    }
    if !abs_direct {
        return None;
    }
    let Some(binding) = abs_symbol.flatten() else {
        return None;
    };
    // Some lowerings reuse the magnitude local as the fraction during bit
    // packing. Only then is a literal store legitimate; a separate magnitude
    // local must never be clobbered by a literal.
    let mut fraction_local = false;
    for node in model.nodes.iter() {
        if !owned(model, node.id(), site) {
            continue;
        }
        match node.kind() {
            AstKind::BinaryExpression(binary)
                if binary.operator.as_str() == "&"
                    && symbol(model, &binary.left) == Some(binding)
                    && is_numeric(&binary.right, 1.) =>
            {
                fraction_local = true;
            }
            AstKind::CallExpression(call) => {
                let Expression::StaticMemberExpression(member) = call.callee.without_parentheses()
                else {
                    continue;
                };
                if member.property.name != "floor" || !path_is(model, &member.object, &["Math"]) {
                    continue;
                }
                let args = call_arguments(call);
                if let Some(Expression::BinaryExpression(division)) = args.first().map(|arg| arg.without_parentheses()) {
                    if division.operator.as_str() == "/"
                        && symbol(model, &division.left) == Some(binding)
                        && is_numeric(&division.right, 2.)
                    {
                        fraction_local = true;
                    }
                }
            }
            _ => {}
        }
    }
    if !fraction_local {
        for node in model.nodes.iter() {
            if !owned(model, node.id(), site) {
                continue;
            }
            let value = match node.kind() {
                AstKind::VariableDeclarator(declarator)
                    if declarator.init.is_some()
                        && declared_symbol(model, declarator) == Some(binding) =>
                {
                    declarator.init.as_ref()
                }
                AstKind::AssignmentExpression(assignment)
                    if assignment.operator.as_str() == "="
                        && assignment_target_symbol(model, &assignment.left) == Some(binding) =>
                {
                    Some(&assignment.right)
                }
                _ => None,
            };
            if value.is_some_and(|value| numeric(value).is_some()) {
                return None;
            }
        }
    }
    if !fraction_select {
        fraction_select = has_fraction_select(model, site);
    }
    if !exponent_carry {
        exponent_carry = has_exponent_carry(model, site);
    }
    if !carry_test {
        carry_test = has_carry_test(model, site);
    }
    let mut installed = false;
    for kind in setup.kinds.values() {
        match kind {
            SetupValue::Float64 => {
                installed = true;
                if !scratch_write || !scratch_buffer {
                    return None;
                }
            }
            SetupValue::Uint8 => {
                installed = true;
                if !byte_constructor {
                    return None;
                }
            }
            SetupValue::Nan => {
                installed = true;
                if !nan_data {
                    return None;
                }
            }
            _ => {}
        }
    }
    if !installed {
        return None;
    }
    let body = loop8_body?;
    let mut final_increment = false;
    for node in model.nodes.iter() {
        if !owned(model, node.id(), site) || node.span().start < body.start || node.span().end > body.end {
            continue;
        }
        if let AstKind::AssignmentExpression(assignment) = node.kind() {
            if let AssignmentTarget::ComputedMemberExpression(member) = &assignment.left {
                if let Expression::UpdateExpression(update) = member.expression.without_parentheses() {
                    if update.prefix && update.operator.as_str() == "++" {
                        final_increment = true;
                    }
                }
            }
        }
    }
    if !final_increment {
        return None;
    }
    if thresholds != BTreeSet::from([32, 4096, 524288, 67108864])
        || shifts != BTreeSet::from([5, 12, 19, 26])
        || powers != BTreeSet::from([-1074, -1022, 52])
        || mod2 < 2
        || !shl7
        || bit_loops < 2
        || lt_half < 2
        || gt_half < 2
        || !xor128
        || !min1023
        || !gt2046
        || !eq2047
        || !bias1023
        || !loop8
        || !sign64
        || !abs_value
        || !fraction_select
        || !exponent_carry
        || !carry_test
    {
        return None;
    }
    Some(())
}

fn numeric_value_alias(model: &Model<'_, '_>, site: FunctionSite, expression: &Expression<'_>) -> bool {
    if model.site_parameter_slot(expression, site) == Some(1) {
        return true;
    }
    let Some(binding) = symbol(model, expression) else {
        return false;
    };
    for node in model.nodes.iter() {
        if !owned(model, node.id(), site) {
            continue;
        }
        let value = match node.kind() {
            AstKind::VariableDeclarator(declarator)
                if declared_symbol(model, declarator) == Some(binding) =>
            {
                declarator.init.as_ref()
            }
            AstKind::AssignmentExpression(assignment)
                if assignment.operator.as_str() == "="
                    && assignment_target_symbol(model, &assignment.left) == Some(binding) =>
            {
                Some(&assignment.right)
            }
            _ => None,
        };
        if value.is_some_and(|value| model.site_parameter_slot(value, site) == Some(1)) {
            return true;
        }
    }
    false
}

/// Composition fallback for the interned-string writer: the shared factory role
/// proves the allocation/return wiring and the nested `writeInternedString`
/// evaluator proves every byte/cache/return effect. The bundle has already
/// proved the stream argument and every writer call in the emitter protocol.
fn writer_role_proof(
    model: &Model<'_, '_>,
    factory: FunctionSite,
    setup: &WrapperSetup,
    number: FunctionSite,
    proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Option<()> {
    if !claimed_role(
        proposals,
        factory.span.start,
        "createInternedStringWriter",
        "shared_scoped_role",
    ) {
        return None;
    }
    let character_code: BTreeSet<Symbol> = setup
        .kinds
        .iter()
        .filter_map(|(binding, kind)| (*kind == SetupValue::CharacterCode).then_some(*binding))
        .collect();
    let number_writer: BTreeSet<Symbol> = setup
        .kinds
        .iter()
        .filter_map(|(binding, kind)| match kind {
            SetupValue::Callable(start) if *start == number.span.start => Some(*binding),
            _ => None,
        })
        .collect();
    if character_code.is_empty() || number_writer.is_empty() {
        return None;
    }
    let (mut char_used, mut number_used) = (false, false);
    for node in model.nodes.iter() {
        if node.span().start < factory.span.start || node.span().end > factory.span.end {
            continue;
        }
        if let AstKind::IdentifierReference(identifier) = node.kind() {
            if let Some(binding) = crate::analysis::identifier_symbol(identifier, model.scoping) {
                char_used |= character_code.contains(&binding);
                number_used |= number_writer.contains(&binding);
            }
        }
    }
    if !char_used || !number_used {
        return None;
    }
    let mut hit_tag = false;
    for node in model.nodes.iter() {
        if node.span().start < factory.span.start || node.span().end > factory.span.end {
            continue;
        }
        if let AstKind::BinaryExpression(binary) = node.kind() {
            if binary.operator.as_str() == "^" && is_numeric(&binary.right, 129.) {
                hit_tag = true;
            }
        }
    }
    if !hit_tag {
        return None;
    }
    let AstKind::Function(function) = model.nodes.kind(factory.node_id) else {
        return None;
    };
    if !plain_parameters(model, function, 1) {
        return None;
    }
    let children = direct_children(model, factory);
    let [writer] = children.as_slice() else {
        return None;
    };
    if !claimed_role(
        proposals,
        writer.span.start,
        "writeInternedString",
        "function_finder::interned_string_writer",
    ) {
        return None;
    }
    Some(())
}

fn protocol_numeric_operations(
    model: &Model<'_, '_>,
    site: FunctionSite,
    setup: &WrapperSetup,
    proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Option<()> {
    if let Some(ports) = numeric_protocol::prove(model, site, &setup.kinds) {
        if setup.kinds.get(&ports.scratch) == Some(&SetupValue::Float64)
            && setup.kinds.get(&ports.byte_constructor) == Some(&SetupValue::Uint8)
            && setup.kinds.get(&ports.nan_data) == Some(&SetupValue::Nan)
        {
            return Some(());
        }
    }
    if !claimed_role(proposals, site.span.start, "writeNumber", "thread_posthost_numeric") {
        return None;
    }
    numeric_semantics_closed(model, site, setup)
}
