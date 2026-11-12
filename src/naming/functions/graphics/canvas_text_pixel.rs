//! Seeded canvas-text pixel emitter (bounded additive family).
//!
//! The owned function is the standalone collector that creates a 96x96 canvas
//! through a captured document-like receiver, schedules one text-sampling task
//! queue over it, registers the collector sink as the queue's `onEmpty`
//! consumer, starts the queue and returns a teardown that empties it:
//!
//! ```text
//! function collect($observer, $document, $queueConfig, $sink) {
//!   let $vector = [];
//!   let $canvas = $document.createElement("canvas");
//!   let $queue = <capturedQueue>($glyph => {
//!     let $canvasLocal = $canvas;
//!     $canvasLocal.height = 96;
//!     $canvasLocal.width = 96;
//!     let $pixels = [];
//!     try {
//!       let $ctx = $canvasLocal.getContext("2d", { willReadFrequently: true });
//!       $ctx.font = $canvasLocal.height - 2 + "px sans-serif";
//!       $ctx.fillStyle = "#000";
//!       $ctx.fillText($glyph[0], -1, $canvasLocal.height);       // or the same
//!       // `[ctx, ctx.fillText][1].call([ctx, ctx.fillText][0], ...)` holder
//!       for (let i = 0; i < $glyph[1].length; i++) {
//!         let $pixel = $ctx.getImageData($glyph[1][i][0], $glyph[1][i][1], 1, 1).data;
//!         $pixels.push($pixel[0] << 24 | $pixel[1] << 16 | $pixel[2] << 8 | $pixel[3]);
//!       }
//!       <arrayPush>.call($vector, $pixels);
//!       let $word = Math.random() * 4294967296;                  // or `| 0`
//!       let $bytes = [$word >>> 24, $word >> 16 & 255, $word >> 8 & 255, $word & 255];
//!       $word = <capturedHash>($word ^ <constant>);
//!       // captured mixer folds $vector and $bytes into the hash state
//!       $observer.next([[], $bytes]);
//!       return void 0;
//!     } catch (e) { /* same completed-vector emission */ }
//!   }, 45, $queueConfig);
//!   $queue.onEmpty(() => { $sink($vector); });                  // or the
//!   // `[queue, queue.onEmpty][1].call([queue, queue.onEmpty][0], ...)` holder
//!   $queue.start();
//!   function teardown() { if ($queue) { $queue.empty(); } }
//!   return teardown;
//! }
//! ```
//!
//! Why this is a distinct family: the already-named `canvas-variants`
//! (`startCanvasPixelProbe` / `handleCanvasProbeImageLoad`) prove a 5x5 `Image`
//! object plus `image.onload` and `drawImage`/5x5 `getImageData`. This family
//! never constructs an image, never sets `onload` and never calls
//! `drawImage`; it draws a caller-supplied glyph with `fillText` and reads one
//! 1x1 pixel per caller-supplied point from the *same* `2d` context it drew on.
//! It is also not the seeded emitter: the field source is the canvas pixel
//! vector, and the random word/reseed is required to reach the captured
//! observer's `next`.
//!
//! The proof is deliberately bounded and additive. It never binds a role from
//! a spelling, thread id, KEY form, vendor, source span, body hash or inherited
//! label. Every participant is a scope-resolved symbol; the only literal
//! values used are the algorithm's own geometry (96), the RGBA shifts and the
//! `"2d"`, `"canvas"`, `"#000"`, `"px sans-serif"`, `willReadFrequently`
//! protocol names. A wrapper that merely forwards to an independently proved
//! nested function is refused because the canvas, context, pixel read, seeded
//! bytes and observer are all proved on the candidate's own body.
//!
//! Aliases and formals are proved one-origin: a followed binding is either a
//! declarator/formal with no writes, or a pre-declared local with exactly one
//! plain assignment and no initializer. Local value definitions must precede
//! their uses. Opaque captured helper identities may be set up after the
//! collector's declaration; alias initializers in that setup still follow
//! definition order. An initializer that a later write overwrites is never
//! followed.
//! The canvas, accumulator and queue must also be defined by the collector's
//! own body, so a sampler-private canvas is refused.

use super::*;
use crate::signals::extract::bindings::{array_elements, numeric_index};
use oxc_semantic::SymbolId;
use oxc_span::GetSpan;

type Symbol = SymbolId;

const ROLE: &str = "emitSeededCanvasTextPixels";
const EVALUATOR: &str = "shared_canvas_text_pixel";

// ---------------------------------------------------------------------------
// Small lexical helpers
// ---------------------------------------------------------------------------

fn symbol(model: &Model<'_, '_>, expression: &Expression<'_>) -> Option<Symbol> {
    crate::analysis::expression_symbol(expression.without_parentheses(), model.scoping)
}

fn is_symbol(model: &Model<'_, '_>, expression: &Expression<'_>, expected: Symbol) -> bool {
    symbol(model, expression) == Some(expected)
}

/// `(object, name)` of a non-optional static member.
fn member<'a>(expression: &'a Expression<'a>, key: &str) -> Option<&'a Expression<'a>> {
    let Expression::StaticMemberExpression(found) = expression.without_parentheses() else {
        return None;
    };
    (!found.optional && found.property.name == key).then_some(&found.object)
}

fn call<'a>(expression: &'a Expression<'a>) -> Option<&'a CallExpression<'a>> {
    match expression.without_parentheses() {
        Expression::CallExpression(found) if !found.optional => Some(found),
        _ => None,
    }
}

fn number(expression: &Expression<'_>, expected: f64) -> bool {
    matches!(
        expression.without_parentheses(),
        Expression::NumericLiteral(literal) if literal.value == expected
    )
}

fn is_numeric(expression: &Expression<'_>) -> bool {
    matches!(
        expression.without_parentheses(),
        Expression::NumericLiteral(_)
    )
}

fn string_value<'a>(expression: &'a Expression<'a>) -> Option<&'a str> {
    match expression.without_parentheses() {
        Expression::StringLiteral(literal) => Some(literal.value.as_str()),
        _ => None,
    }
}

fn block<'a>(statement: &'a Statement<'a>) -> Option<&'a [Statement<'a>]> {
    let Statement::BlockStatement(found) = statement else {
        return None;
    };
    Some(found.body.as_slice())
}

fn binding_symbol(pattern: &BindingPattern<'_>) -> Option<Symbol> {
    let BindingPattern::BindingIdentifier(identifier) = pattern else {
        return None;
    };
    identifier.symbol_id.get()
}

fn empty_array(expression: &Expression<'_>) -> bool {
    array_elements(expression.without_parentheses()).is_some_and(|elements| elements.is_empty())
}

fn pushed_identifier<'a>(target: &'a AssignmentTarget<'a>) -> Option<&'a IdentifierReference<'a>> {
    let AssignmentTarget::AssignmentTargetIdentifier(identifier) = target else {
        return None;
    };
    Some(identifier)
}

/// A binding that is read as one stable value: a declarator initializer or an
/// incoming formal that is never written, or the pre-declared form
/// (`let x; x = value;`) with exactly one plain `=` write and no initializer.
/// An initializer that a later write overwrites is not the binding's value, so
/// the first resolution is refused rather than followed.
fn single_origin(model: &Model<'_, '_>, symbol: Symbol) -> bool {
    let writes = model.binding_writes(symbol);
    if writes == 0 {
        return true;
    }
    writes == 1 && model.initializer(symbol).is_none() && model.symbol_value(symbol).is_some()
}

/// A definition must start before the use it feeds. At or after the use the
/// binding is read in its temporal dead zone, so the collector cannot execute.
/// Shared by the alias-chain walkers (`root_symbol`/`slot_of`) and the
/// direct-use holder and push-alias identities, which are consumed by symbol
/// identity rather than followed through those walkers.
fn defined_before(value: &Expression<'_>, use_start: u32) -> bool {
    value.span().start < use_start
}

/// The binding's one value is defined by a statement owned by `site` itself,
/// never by a nested arrow or function body.
fn defined_in(model: &Model<'_, '_>, site: FunctionSite, symbol: Symbol) -> bool {
    if model.initializer(symbol).is_some() {
        return model
            .owning_site(model.scoping.symbol_declaration(symbol))
            .is_some_and(|owner| owner.span == site.span);
    }
    let mut writes = model
        .scoping
        .get_resolved_references(symbol)
        .filter(|reference| reference.is_write());
    let Some(reference) = writes.next() else {
        return false;
    };
    writes.next().is_none()
        && model
            .owning_site(reference.node_id())
            .is_some_and(|owner| owner.span == site.span)
}

/// Follow one-origin plain alias chains (`let a = b; let b = c;`). Every
/// followed binding must be single-origin and its definition must precede the
/// use, so an overwritten alias is never resolved to its first initializer.
/// The last resolved symbol is returned; a member expression or literal stops
/// the walk.
fn root_symbol(model: &Model<'_, '_>, expression: &Expression<'_>) -> Option<Symbol> {
    let mut current = expression.without_parentheses();
    let mut last = None;
    for _ in 0..8 {
        let Some(found) = symbol(model, current) else {
            break;
        };
        if !single_origin(model, found) {
            return None;
        }
        last = Some(found);
        let Some(value) = model.symbol_value(found) else {
            break;
        };
        if !defined_before(value, current.span().start) {
            return None;
        }
        current = value.without_parentheses();
    }
    last
}

/// The outer function formal slot an expression denotes, through single-origin
/// plain locals that only carry that same slot and are defined before use.
fn slot_of(model: &Model<'_, '_>, expression: &Expression<'_>, site: FunctionSite) -> Option<u32> {
    let mut current = expression.without_parentheses();
    for _ in 0..8 {
        if let Some(slot) = model.site_parameter_slot(current, site) {
            return Some(slot);
        }
        let found = symbol(model, current)?;
        if !single_origin(model, found) {
            return None;
        }
        let value = model.symbol_value(found)?;
        if !defined_before(value, current.span().start) {
            return None;
        }
        current = value.without_parentheses();
    }
    None
}

fn captured(model: &Model<'_, '_>, site: FunctionSite, found: Symbol) -> bool {
    !site
        .span
        .contains_inclusive(model.scoping.symbol_span(found))
        && model.scoping.symbol_redeclarations(found).is_empty()
}

/// Compare opaque captured helpers by their stable identity. Their enclosing
/// setup may follow the collector's declaration: a use inside its deferred
/// sampler is not an immediate read at that source position. Only that first
/// edge crosses a function boundary. Subsequent alias initializers execute in
/// setup order, so their definitions must still precede the reads they feed.
/// A nested setter cannot donate the captured binding's definition.
fn captured_callable_root(
    model: &Model<'_, '_>,
    site: FunctionSite,
    expression: &Expression<'_>,
) -> Option<Symbol> {
    let mut current = expression;
    for depth in 0..8 {
        let found = symbol(model, current)?;
        if !captured(model, site, found) || !single_origin(model, found) {
            return None;
        }
        let Some(value) = model.symbol_value(found) else {
            return Some(found);
        };
        let owner = model.owning_site(model.scoping.symbol_declaration(found))?;
        if !defined_in(model, owner, found)
            || (depth != 0 && !defined_before(value, current.span().start))
        {
            return None;
        }
        if symbol(model, value).is_none() {
            return Some(found);
        }
        current = value;
    }
    None
}

/// The symbol assigned a call result (`let q = f(...)` or `q = f(...)`).
fn call_binding(model: &Model<'_, '_>, call: &CallExpression<'_>) -> Option<Symbol> {
    let parent = model.nodes.parent_id(call.node_id.get());
    match model.nodes.kind(parent) {
        AstKind::VariableDeclarator(declarator)
            if declarator
                .init
                .as_ref()
                .is_some_and(|init| init.span() == call.span) =>
        {
            binding_symbol(&declarator.id)
        }
        AstKind::AssignmentExpression(assignment)
            if assignment.operator.as_str() == "=" && assignment.right.span() == call.span =>
        {
            let identifier = pushed_identifier(&assignment.left)?;
            crate::analysis::identifier_symbol(identifier, model.scoping)
        }
        _ => None,
    }
}

/// `holder[index].call(...)`.
fn holder_method_call<'a>(
    model: &Model<'_, '_>,
    call: &'a CallExpression<'a>,
) -> Option<(Symbol, u32, Vec<&'a Expression<'a>>)> {
    let Expression::StaticMemberExpression(method) = call.callee.without_parentheses() else {
        return None;
    };
    if method.optional || method.property.name != "call" {
        return None;
    }
    let Expression::ComputedMemberExpression(indexed) = method.object.without_parentheses() else {
        return None;
    };
    if indexed.optional {
        return None;
    }
    let index = numeric_index(&indexed.expression)?;
    let holder = symbol(model, &indexed.object)?;
    Some((holder, index, call_arguments(call)))
}

/// `holder[index]` with the exact same local holder.
fn holder_index(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    holder: Symbol,
    index: u32,
) -> bool {
    let Expression::ComputedMemberExpression(found) = expression.without_parentheses() else {
        return false;
    };
    !found.optional
        && numeric_index(&found.expression) == Some(index)
        && is_symbol(model, &found.object, holder)
}

fn plain_formals(
    model: &Model<'_, '_>,
    function: &Function<'_>,
    arity: usize,
) -> Option<Vec<Symbol>> {
    if function.params.rest.is_some() || function.params.items.len() != arity {
        return None;
    }
    let mut formals = Vec::new();
    for parameter in &function.params.items {
        if parameter.initializer.is_some() {
            return None;
        }
        let found = binding_symbol(&parameter.pattern)?;
        if !model.scoping.symbol_redeclarations(found).is_empty() {
            return None;
        }
        // A written formal no longer denotes its incoming slot argument.
        if model.binding_writes(found) != 0 {
            return None;
        }
        formals.push(found);
    }
    Some(formals)
}

// ---------------------------------------------------------------------------
// Seeded-pixel dataflow helpers
// ---------------------------------------------------------------------------

fn is_math_random(model: &Model<'_, '_>, expression: &Expression<'_>) -> bool {
    let Some(random) = call(expression) else {
        return false;
    };
    if !random.arguments.is_empty() {
        return false;
    }
    let Some(receiver) = member(&random.callee, "random") else {
        return false;
    };
    let Expression::Identifier(identifier) = receiver.without_parentheses() else {
        return false;
    };
    identifier.name == "Math"
        && crate::analysis::identifier_symbol(identifier, model.scoping).is_none()
}

/// `Math.random() * 4294967296`, optionally truncated with `| 0`.
fn is_random_32(model: &Model<'_, '_>, expression: &Expression<'_>) -> bool {
    let expression = expression.without_parentheses();
    let product = match expression {
        Expression::BinaryExpression(bit_or) if bit_or.operator.as_str() == "|" => {
            if !number(&bit_or.right, 0.0) {
                return false;
            }
            bit_or.left.without_parentheses()
        }
        _ => expression,
    };
    let Expression::BinaryExpression(product) = product else {
        return false;
    };
    product.operator.as_str() == "*"
        && is_math_random(model, &product.left)
        && number(&product.right, 4294967296.0)
}

/// One `word >>> shift` or `word >> shift` lane of the random word.
fn shifted_lane(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    random: Symbol,
    operator: &str,
    shift: f64,
) -> bool {
    let Expression::BinaryExpression(binary) = expression.without_parentheses() else {
        return false;
    };
    binary.operator.as_str() == operator
        && number(&binary.right, shift)
        && coerced_word(model, &binary.left, random)
}

/// One `word >> shift & 255` lane of the random word.
fn masked_shift_lane(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    random: Symbol,
    shift: f64,
) -> bool {
    let Expression::BinaryExpression(binary) = expression.without_parentheses() else {
        return false;
    };
    binary.operator.as_str() == "&"
        && number(&binary.right, 255.0)
        && shifted_lane(model, &binary.left, random, ">>", shift)
}

/// The `word & 255` low lane of the random word.
fn masked_lane(model: &Model<'_, '_>, expression: &Expression<'_>, random: Symbol) -> bool {
    let Expression::BinaryExpression(binary) = expression.without_parentheses() else {
        return false;
    };
    binary.operator.as_str() == "&"
        && number(&binary.right, 255.0)
        && coerced_word(model, &binary.left, random)
}

/// The exact four RGBA byte lanes of the random word, in order:
/// `word >>> 24, word >> 16 & 255, word >> 8 & 255, word & 255`. A four-element
/// array that merely mentions the word is not a byte-lane derivation.
fn byte_lanes(model: &Model<'_, '_>, elements: &[&Expression<'_>], random: Symbol) -> bool {
    if elements.len() != 4 {
        return false;
    }
    shifted_lane(model, elements[0], random, ">>>", 24.0)
        && masked_shift_lane(model, elements[1], random, 16.0)
        && masked_shift_lane(model, elements[2], random, 8.0)
        && masked_lane(model, elements[3], random)
}

/// The random binding itself, or the same binding truncated with `| 0` at a
/// use site (either operand order).
fn coerced_word(model: &Model<'_, '_>, expression: &Expression<'_>, random: Symbol) -> bool {
    let expression = expression.without_parentheses();
    if is_symbol(model, expression, random) {
        return true;
    }
    let Expression::BinaryExpression(binary) = expression else {
        return false;
    };
    binary.operator.as_str() == "|"
        && ((is_symbol(model, &binary.left, random) && number(&binary.right, 0.0))
            || (number(&binary.left, 0.0) && is_symbol(model, &binary.right, random)))
}

/// `random ^ <integer literal>` on either side.
fn is_random_xor(model: &Model<'_, '_>, expression: &Expression<'_>, random: Symbol) -> bool {
    let Expression::BinaryExpression(binary) = expression.without_parentheses() else {
        return false;
    };
    binary.operator.as_str() == "^"
        && ((coerced_word(model, &binary.left, random) && is_numeric(&binary.right))
            || (is_numeric(&binary.left) && coerced_word(model, &binary.right, random)))
}

/// The four RGBA lanes of one `getImageData(...).data` binding packed into one
/// int32: `d[0] << 24 | d[1] << 16 | d[2] << 8 | d[3]`.
fn rgba_lanes(model: &Model<'_, '_>, expression: &Expression<'_>, data: Symbol) -> Option<u8> {
    match expression.without_parentheses() {
        Expression::BinaryExpression(binary) if binary.operator.as_str() == "|" => {
            let left = rgba_lanes(model, &binary.left, data)?;
            let right = rgba_lanes(model, &binary.right, data)?;
            // Every leaf must be a known lane, appearing exactly once.
            (left & right == 0).then_some(left | right)
        }
        Expression::BinaryExpression(binary) if binary.operator.as_str() == "<<" => {
            let lane = |index: u32, shift: f64| {
                is_lane(model, &binary.left, data, index) && number(&binary.right, shift)
            };
            if lane(0, 24.0) {
                Some(0b0001)
            } else if lane(1, 16.0) {
                Some(0b0010)
            } else if lane(2, 8.0) {
                Some(0b0100)
            } else {
                None
            }
        }
        Expression::ComputedMemberExpression(_) if is_lane(model, expression, data, 3) => {
            Some(0b1000)
        }
        _ => None,
    }
}

fn is_lane(model: &Model<'_, '_>, expression: &Expression<'_>, data: Symbol, index: u32) -> bool {
    let Expression::ComputedMemberExpression(found) = expression.without_parentheses() else {
        return false;
    };
    !found.optional
        && numeric_index(&found.expression) == Some(index)
        && is_symbol(model, &found.object, data)
}

/// `message[1][index][lane]`, returning the shared point index.
fn point_lane<'a>(
    model: &Model<'_, '_>,
    expression: &'a Expression<'a>,
    message: Symbol,
    lane: u32,
) -> Option<&'a Expression<'a>> {
    let Expression::ComputedMemberExpression(outer) = expression.without_parentheses() else {
        return None;
    };
    if outer.optional || numeric_index(&outer.expression) != Some(lane) {
        return None;
    }
    let Expression::ComputedMemberExpression(point) = outer.object.without_parentheses() else {
        return None;
    };
    if point.optional {
        return None;
    }
    let Expression::ComputedMemberExpression(list) = point.object.without_parentheses() else {
        return None;
    };
    if list.optional || numeric_index(&list.expression) != Some(1) {
        return None;
    }
    if !is_symbol(model, &list.object, message) {
        return None;
    }
    Some(&point.expression)
}

fn same_index(model: &Model<'_, '_>, left: &Expression<'_>, right: &Expression<'_>) -> bool {
    match (left.without_parentheses(), right.without_parentheses()) {
        (Expression::Identifier(_), Expression::Identifier(_)) => {
            symbol(model, left).is_some() && symbol(model, left) == symbol(model, right)
        }
        (Expression::NumericLiteral(a), Expression::NumericLiteral(b)) => a.value == b.value,
        _ => false,
    }
}

fn will_read_frequently(expression: &Expression<'_>) -> bool {
    let Expression::ObjectExpression(object) = expression.without_parentheses() else {
        return false;
    };
    let [property] = object.properties.as_slice() else {
        return false;
    };
    let ObjectPropertyKind::ObjectProperty(property) = property else {
        return false;
    };
    !property.computed
        && !property.method
        && !property.shorthand
        && property_key_name(&property.key).as_deref() == Some("willReadFrequently")
        && matches!(&property.value, Expression::BooleanLiteral(literal) if literal.value)
}

fn font_value(model: &Model<'_, '_>, expression: &Expression<'_>, canvas: Symbol) -> bool {
    let Expression::BinaryExpression(plus) = expression.without_parentheses() else {
        return false;
    };
    if plus.operator.as_str() != "+" || string_value(&plus.right) != Some("px sans-serif") {
        return false;
    }
    let Expression::BinaryExpression(minus) = plus.left.without_parentheses() else {
        return false;
    };
    minus.operator.as_str() == "-"
        && number(&minus.right, 2.0)
        && member(&minus.left, "height")
            .is_some_and(|object| root_symbol(model, object) == Some(canvas))
}

// ---------------------------------------------------------------------------
// Own-body scan
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Scan<'a> {
    calls: Vec<&'a CallExpression<'a>>,
    declarators: Vec<&'a VariableDeclarator<'a>>,
    assignments: Vec<&'a AssignmentExpression<'a>>,
}

/// Every call, declarator and plain assignment owned directly by `site`, never
/// one owned by a nested function or arrow.
fn scan_sampler<'a>(model: &Model<'_, 'a>, site: FunctionSite) -> Scan<'a> {
    let mut scan = Scan::default();
    let Some(body) = site.body_span else {
        return scan;
    };
    for node in model.nodes.iter() {
        let span = node.span();
        if span.start < body.start || span.end > body.end {
            continue;
        }
        if !model
            .owning_site(node.id())
            .is_some_and(|owner| owner.span == site.span)
        {
            continue;
        }
        match node.kind() {
            AstKind::CallExpression(found) => scan.calls.push(found),
            AstKind::VariableDeclarator(found) => scan.declarators.push(found),
            AstKind::AssignmentExpression(found) => scan.assignments.push(found),
            _ => {}
        }
    }
    scan
}

fn values<'a>(model: &Model<'_, 'a>, scan: &Scan<'a>) -> Vec<(Symbol, &'a Expression<'a>)> {
    let mut out = Vec::new();
    for reference in &scan.declarators {
        let declarator: &'a VariableDeclarator<'a> = *reference;
        let (Some(init), Some(found)) = (declarator.init.as_ref(), binding_symbol(&declarator.id))
        else {
            continue;
        };
        out.push((found, init));
    }
    for reference in &scan.assignments {
        let assignment: &'a AssignmentExpression<'a> = *reference;
        if assignment.operator.as_str() != "=" {
            continue;
        }
        let Some(identifier) = pushed_identifier(&assignment.left) else {
            continue;
        };
        let Some(found) = crate::analysis::identifier_symbol(identifier, model.scoping) else {
            continue;
        };
        out.push((found, &assignment.right));
    }
    out
}

// ---------------------------------------------------------------------------
// Sampler proof
// ---------------------------------------------------------------------------

struct Sampler {
    canvas: Symbol,
    array: Symbol,
    message: Symbol,
    context: Symbol,
    pixels: Symbol,
}

fn canvas_size(model: &Model<'_, '_>, scan: &Scan<'_>) -> Option<Symbol> {
    let mut sizes: BTreeMap<Symbol, (bool, bool)> = BTreeMap::new();
    for assignment in &scan.assignments {
        let (height, width, object) = match &assignment.left {
            AssignmentTarget::StaticMemberExpression(found) if !found.optional => {
                match found.property.name.as_str() {
                    "height" => (true, false, &found.object),
                    "width" => (false, true, &found.object),
                    _ => continue,
                }
            }
            _ => continue,
        };
        if !number(&assignment.right, 96.0) {
            continue;
        }
        let Some(root) = root_symbol(model, object) else {
            continue;
        };
        let entry = sizes.entry(root).or_default();
        entry.0 |= height;
        entry.1 |= width;
    }
    let mut canvas = None;
    for (root, (height, width)) in sizes {
        if height && width {
            if canvas.is_some() {
                return None;
            }
            canvas = Some(root);
        }
    }
    canvas
}

fn canvas_context(
    model: &Model<'_, '_>,
    entries: &[(Symbol, &Expression<'_>)],
    canvas: Symbol,
) -> Option<Symbol> {
    let mut context = None;
    for (found, value) in entries {
        let Some(found_call) = call(value) else {
            continue;
        };
        let Some(receiver) = member(&found_call.callee, "getContext") else {
            continue;
        };
        if root_symbol(model, receiver) != Some(canvas) {
            continue;
        }
        let arguments = call_arguments(found_call);
        let [kind, options] = arguments.as_slice() else {
            continue;
        };
        if string_value(kind) != Some("2d") || !will_read_frequently(options) {
            continue;
        }
        if !single_origin(model, *found) {
            continue;
        }
        if context.is_some_and(|previous| previous != *found) {
            return None;
        }
        context = Some(*found);
    }
    context
}

fn draws_text(
    model: &Model<'_, '_>,
    scan: &Scan<'_>,
    entries: &[(Symbol, &Expression<'_>)],
    context: Symbol,
    message: Symbol,
) -> bool {
    for found in &scan.calls {
        let Some(receiver) = member(&found.callee, "fillText") else {
            continue;
        };
        if root_symbol(model, receiver) != Some(context) {
            continue;
        }
        if call_arguments(found)
            .first()
            .is_some_and(|argument| is_lane(model, argument, message, 0))
        {
            return true;
        }
    }
    // `[ctx, ctx.fillText][1].call([ctx, ctx.fillText][0], glyph, ...)`.
    for (holder, value) in entries {
        if !single_origin(model, *holder) {
            continue;
        }
        let Some(elements) = array_elements(value.without_parentheses()) else {
            continue;
        };
        let [context_argument, method] = elements.as_slice() else {
            continue;
        };
        if root_symbol(model, context_argument) != Some(context) {
            continue;
        }
        let Some(fill_text_receiver) = member(method, "fillText") else {
            continue;
        };
        if root_symbol(model, fill_text_receiver) != Some(context) {
            continue;
        }
        for found in &scan.calls {
            let Some((found_holder, index, arguments)) = holder_method_call(model, found) else {
                continue;
            };
            if found_holder != *holder || index != 1 || arguments.len() < 2 {
                continue;
            }
            if !defined_before(*value, found.span.start) {
                continue;
            }
            if holder_index(model, arguments[0], *holder, 0)
                && is_lane(model, arguments[1], message, 0)
            {
                return true;
            }
        }
    }
    false
}

fn pixel_data<'a>(
    model: &Model<'_, 'a>,
    entries: &[(Symbol, &'a Expression<'a>)],
    context: Symbol,
    message: Symbol,
) -> Option<Symbol> {
    let mut data = None;
    for (found, value) in entries {
        let Some(get_image) = member(value, "data") else {
            continue;
        };
        let Some(get_call) = call(get_image) else {
            continue;
        };
        let Some(receiver) = member(&get_call.callee, "getImageData") else {
            continue;
        };
        if root_symbol(model, receiver) != Some(context) {
            continue;
        }
        let arguments = call_arguments(get_call);
        let [x, y, width, height] = arguments.as_slice() else {
            continue;
        };
        if !number(width, 1.0) || !number(height, 1.0) {
            continue;
        }
        let (Some(x_index), Some(y_index)) = (
            point_lane(model, x, message, 0),
            point_lane(model, y, message, 1),
        ) else {
            continue;
        };
        if !same_index(model, x_index, y_index) {
            continue;
        }
        if !single_origin(model, *found) {
            continue;
        }
        if data.is_some_and(|previous| previous != *found) {
            return None;
        }
        data = Some(*found);
    }
    data
}

fn pixel_vector(model: &Model<'_, '_>, scan: &Scan<'_>, data: Symbol) -> Option<Symbol> {
    let mut pixels = None;
    for found in &scan.calls {
        let Some(receiver) = member(&found.callee, "push") else {
            continue;
        };
        let arguments = call_arguments(found);
        let [value] = arguments.as_slice() else {
            continue;
        };
        if found.arguments.len() != 1 || rgba_lanes(model, value, data) != Some(0b1111) {
            continue;
        }
        let Some(root) = root_symbol(model, receiver) else {
            continue;
        };
        let Some(value) = model.symbol_value(root) else {
            continue;
        };
        if !empty_array(value) {
            continue;
        }
        if pixels.is_some_and(|previous| previous != root) {
            return None;
        }
        pixels = Some(root);
    }
    pixels
}

/// The outer accumulator pushed with the pixel vector, in either the
/// `pushAlias.call(array, pixels)` or the direct `array.push(pixels)` spelling.
fn accumulator(
    model: &Model<'_, '_>,
    scan: &Scan<'_>,
    entries: &[(Symbol, &Expression<'_>)],
    pixels: Symbol,
) -> Option<Symbol> {
    let mut array = None;
    for (alias, value) in entries {
        if !single_origin(model, *alias) {
            continue;
        }
        let Some(receiver) = member(value, "push") else {
            continue;
        };
        let Some(root) = root_symbol(model, receiver) else {
            continue;
        };
        if root == pixels {
            continue;
        }
        for found in &scan.calls {
            let Expression::StaticMemberExpression(method) = found.callee.without_parentheses()
            else {
                continue;
            };
            if method.optional || method.property.name != "call" {
                continue;
            }
            if !is_symbol(model, &method.object, *alias) {
                continue;
            }
            if !defined_before(*value, found.span.start) {
                continue;
            }
            let arguments = call_arguments(found);
            let [array_argument, value_argument] = arguments.as_slice() else {
                continue;
            };
            if root_symbol(model, array_argument) == Some(root)
                && root_symbol(model, value_argument) == Some(pixels)
            {
                array = Some(root);
            }
        }
    }
    // The receiver may capture the collector's array, whose definition is not
    // in the sampler-local entries. Resolve it from the actual direct call;
    // the caller still requires this root to be defined by the outer collector.
    for found_call in &scan.calls {
        let Some(receiver) = member(&found_call.callee, "push") else {
            continue;
        };
        let Some(root) = root_symbol(model, receiver) else {
            continue;
        };
        if root == pixels || !array_created(model, root) {
            continue;
        }
        let Some(arguments) = exact_arguments(found_call, 1) else {
            continue;
        };
        if root_symbol(model, arguments[0]) == Some(pixels) {
            array = Some(root);
        }
    }
    array
}

/// These readers consume whole statements, so nested functions, dead donor
/// blocks and sibling arms cannot supply an edge in the encoding path.
fn statement_expression<'a>(statement: &'a Statement<'a>) -> Option<&'a Expression<'a>> {
    let Statement::ExpressionStatement(found) = statement else {
        return None;
    };
    Some(&found.expression)
}

fn local_value<'a>(statement: &'a Statement<'a>) -> Option<(Symbol, &'a Expression<'a>)> {
    let Statement::VariableDeclaration(found) = statement else {
        return None;
    };
    let [declaration] = found.declarations.as_slice() else {
        return None;
    };
    Some((binding_symbol(&declaration.id)?, declaration.init.as_ref()?))
}

fn assigned_value<'a>(
    model: &Model<'_, '_>,
    statement: &'a Statement<'a>,
    target: Symbol,
) -> Option<&'a Expression<'a>> {
    let Expression::AssignmentExpression(found) =
        statement_expression(statement)?.without_parentheses()
    else {
        return None;
    };
    let identifier = pushed_identifier(&found.left)?;
    (found.operator.as_str() == "="
        && crate::analysis::identifier_symbol(identifier, model.scoping) == Some(target))
    .then_some(&found.right)
}

fn exact_arguments<'a>(
    found: &'a CallExpression<'a>,
    count: usize,
) -> Option<Vec<&'a Expression<'a>>> {
    let arguments = call_arguments(found);
    (found.arguments.len() == count && arguments.len() == count).then_some(arguments)
}

fn indexed_by(model: &Model<'_, '_>, value: &Expression<'_>, array: Symbol, index: Symbol) -> bool {
    let Expression::ComputedMemberExpression(found) = value.without_parentheses() else {
        return false;
    };
    !found.optional
        && is_symbol(model, &found.object, array)
        && is_symbol(model, &found.expression, index)
}

fn length_of(model: &Model<'_, '_>, value: &Expression<'_>, array: Symbol) -> bool {
    member(value, "length").is_some_and(|object| is_symbol(model, object, array))
}

fn decrement(model: &Model<'_, '_>, statement: &Statement<'_>, index: Symbol) -> bool {
    let Some(expression) = statement_expression(statement) else {
        return false;
    };
    let Expression::UpdateExpression(found) = expression.without_parentheses() else {
        return false;
    };
    let SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier) = &found.argument else {
        return false;
    };
    found.prefix
        && found.operator.as_str() == "--"
        && crate::analysis::identifier_symbol(identifier, model.scoping) == Some(index)
}

fn exhausted_break(model: &Model<'_, '_>, statement: &Statement<'_>, index: Symbol) -> bool {
    let Statement::IfStatement(found) = statement else {
        return false;
    };
    let Expression::UnaryExpression(not) = found.test.without_parentheses() else {
        return false;
    };
    let Expression::BinaryExpression(test) = not.argument.without_parentheses() else {
        return false;
    };
    found.alternate.is_none()
        && not.operator.as_str() == "!"
        && test.operator.as_str() == ">="
        && is_symbol(model, &test.left, index)
        && number(&test.right, 0.0)
        && matches!(&found.consequent, Statement::BreakStatement(exit) if exit.label.is_none())
}

fn number_index(model: &Model<'_, '_>, statement: &Statement<'_>, index: Symbol) -> bool {
    let Some(value) = assigned_value(model, statement, index) else {
        return false;
    };
    let Some(found) = call(value) else {
        return false;
    };
    let Expression::Identifier(identifier) = found.callee.without_parentheses() else {
        return false;
    };
    identifier.name == "Number"
        && crate::analysis::identifier_symbol(identifier, model.scoping).is_none()
        && exact_arguments(found, 1).is_some_and(|args| is_symbol(model, args[0], index))
}

fn unbounded_loop<'a>(statement: &'a Statement<'a>) -> Option<&'a [Statement<'a>]> {
    let Statement::ForStatement(found) = statement else {
        return None;
    };
    if found.init.is_some() || found.test.is_some() || found.update.is_some() {
        return None;
    }
    block(&found.body)
}

/// A captured numeric mixer writes into the same header and threads its
/// returned cursor through every subsequent write. Its implementation is an
/// opaque protocol input here, just as the captured reseeding hash is.
fn mixer_arguments<'a>(
    model: &Model<'_, '_>,
    value: &'a Expression<'a>,
    mixer: Symbol,
    state: Symbol,
    bytes: Symbol,
) -> Option<Vec<&'a Expression<'a>>> {
    let found = call(value)?;
    let arguments = exact_arguments(found, 4)?;
    (is_symbol(model, &found.callee, mixer)
        && is_symbol(model, arguments[0], state)
        && is_symbol(model, arguments[2], bytes))
    .then_some(arguments)
}

fn mixer_loop(
    model: &Model<'_, '_>,
    statement: &Statement<'_>,
    array: Symbol,
    count: Symbol,
    forward_index: Option<Symbol>,
    cursor: Symbol,
    mixer: Symbol,
    state: Symbol,
    bytes: Symbol,
) -> Option<()> {
    let (rows, index) = if let Some(index) = forward_index {
        let Statement::WhileStatement(found) = statement else {
            return None;
        };
        if !less_than(model, &found.test, index, count) {
            return None;
        }
        let body = block(&found.body)?;
        let (step, rows) = body.split_last()?;
        if !increment(model, statement_expression(step)?, index) {
            return None;
        }
        (rows, index)
    } else {
        let [step, guard, rows @ .., normalize] = unbounded_loop(statement)? else {
            return None;
        };
        if !decrement(model, step, count)
            || !exhausted_break(model, guard, count)
            || !number_index(model, normalize, count)
        {
            return None;
        }
        (rows, count)
    };
    let (row, size, inner_index, row_write, inner) = match rows {
        [row, size, row_write, inner] => (row, size, None, row_write, inner),
        [row, size, init, row_write, inner] => {
            let (index, value) = local_value(init)?;
            if !number(value, 0.0) || model.binding_writes(index) != 1 {
                return None;
            }
            (row, size, Some(index), row_write, inner)
        }
        _ => return None,
    };
    let (row, row_value) = local_value(row)?;
    let (size, size_value) = local_value(size)?;
    if !single_origin(model, row)
        || !indexed_by(model, row_value, array, index)
        || !length_of(model, size_value, row)
        || model.binding_writes(size) != (if inner_index.is_some() { 0 } else { 2 })
    {
        return None;
    }
    let arguments = mixer_arguments(
        model,
        assigned_value(model, row_write, cursor)?,
        mixer,
        state,
        bytes,
    )?;
    if !is_symbol(model, arguments[1], size) || !is_symbol(model, arguments[3], cursor) {
        return None;
    }
    let (write, pixel_index) = if let Some(index) = inner_index {
        let Statement::ForStatement(found) = inner else {
            return None;
        };
        if found.init.is_some()
            || !less_than(model, found.test.as_ref()?, index, size)
            || !increment(model, found.update.as_ref()?, index)
        {
            return None;
        }
        let [write] = block(&found.body)? else {
            return None;
        };
        (write, index)
    } else {
        let [step, guard, write, normalize] = unbounded_loop(inner)? else {
            return None;
        };
        if !decrement(model, step, size)
            || !exhausted_break(model, guard, size)
            || !number_index(model, normalize, size)
        {
            return None;
        }
        (write, size)
    };
    let arguments = mixer_arguments(
        model,
        assigned_value(model, write, cursor)?,
        mixer,
        state,
        bytes,
    )?;
    if !indexed_by(model, arguments[1], row, pixel_index) || !is_symbol(model, arguments[3], cursor)
    {
        return None;
    }
    // Distinct bindings prevent a header, seed, cursor or loop counter from
    // donating a value to another position in the protocol.
    let mut symbols = vec![array, count, row, size, cursor, state, bytes, mixer];
    symbols.extend(forward_index);
    symbols.extend(inner_index);
    for (index, symbol) in symbols.iter().enumerate() {
        if symbols[..index].contains(symbol) {
            return None;
        }
    }
    Some(())
}

fn less_than(model: &Model<'_, '_>, value: &Expression<'_>, index: Symbol, count: Symbol) -> bool {
    matches!(value.without_parentheses(), Expression::BinaryExpression(test)
        if test.operator.as_str() == "<" && is_symbol(model, &test.left, index)
            && is_symbol(model, &test.right, count))
}

fn increment(model: &Model<'_, '_>, value: &Expression<'_>, index: Symbol) -> bool {
    let Expression::UpdateExpression(found) = value.without_parentheses() else {
        return false;
    };
    let SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier) = &found.argument else {
        return false;
    };
    found.operator.as_str() == "++"
        && crate::analysis::identifier_symbol(identifier, model.scoping) == Some(index)
}

/// The outer forward loop has one extra, ordered zero-index declaration.
/// This only selects a bounded shape; seeded_arm must validate every edge.
fn seeded_tail<'a>(statements: &'a [Statement<'a>]) -> Option<&'a [Statement<'a>]> {
    let length = match statements.get(statements.len().checked_sub(3)?)? {
        Statement::WhileStatement(_) => 10,
        Statement::ForStatement(_) => 9,
        _ => return None,
    };
    statements.get(statements.len().checked_sub(length)?..)
}

fn seeded_arm(
    model: &Model<'_, '_>,
    statements: &[Statement<'_>],
    outer_site: FunctionSite,
    array: Symbol,
    pixels: Symbol,
    entries: &[(Symbol, &Expression<'_>)],
) -> Option<()> {
    // The push must immediately feed this arm's encoding, rather than merely
    // occur elsewhere in the sampler. Earlier statements draw/read pixels.
    let (push, random, header, reseed, count, index, cursor, encode, emit, exit) =
        match seeded_tail(statements)? {
            [push, random, header, reseed, count, cursor, encode, emit, exit] => (
                push, random, header, reseed, count, None, cursor, encode, emit, exit,
            ),
            [push, random, header, reseed, count, index, cursor, encode, emit, exit] => (
                push,
                random,
                header,
                reseed,
                count,
                Some(index),
                cursor,
                encode,
                emit,
                exit,
            ),
            _ => return None,
        };
    let push = call(statement_expression(push)?)?;
    let direct_push = member(&push.callee, "push").is_some_and(|receiver| {
        root_symbol(model, receiver) == Some(array)
            && exact_arguments(push, 1)
                .is_some_and(|args| root_symbol(model, args[0]) == Some(pixels))
    });
    let alias_push = member(&push.callee, "call").is_some_and(|callee| {
        let Some(alias) = symbol(model, callee) else {
            return false;
        };
        entries.iter().any(|(found, value)| {
            *found == alias
                && single_origin(model, alias)
                && defined_before(value, push.span.start)
                && member(value, "push")
                    .is_some_and(|receiver| root_symbol(model, receiver) == Some(array))
                && exact_arguments(push, 2).is_some_and(|args| {
                    root_symbol(model, args[0]) == Some(array)
                        && root_symbol(model, args[1]) == Some(pixels)
                })
        })
    });
    if !direct_push && !alias_push {
        return None;
    }
    let (random, random_value) = local_value(random)?;
    let (bytes, header_value) = local_value(header)?;
    if !is_random_32(model, random_value)
        || model.binding_writes(random) != 1
        || !single_origin(model, bytes)
        || !byte_lanes(
            model,
            &array_elements(header_value.without_parentheses())?,
            random,
        )
    {
        return None;
    }
    let reseed = call(assigned_value(model, reseed, random)?)?;
    let hash = symbol(model, &reseed.callee)?;
    let arguments = exact_arguments(reseed, 1)?;
    if !captured(model, outer_site, hash) || !is_random_xor(model, arguments[0], random) {
        return None;
    }
    let (count, count_value) = local_value(count)?;
    let forward_index = if let Some(init) = index {
        let (index, value) = local_value(init)?;
        if !number(value, 0.0) || model.binding_writes(index) != 1 {
            return None;
        }
        Some(index)
    } else {
        None
    };
    let (cursor, cursor_value) = local_value(cursor)?;
    if !length_of(model, count_value, array)
        || model.binding_writes(count) != (if forward_index.is_some() { 0 } else { 2 })
        || model.binding_writes(cursor) != 2
    {
        return None;
    }
    let first = call(cursor_value)?;
    let mixer = symbol(model, &first.callee)?;
    if !captured(model, outer_site, mixer) {
        return None;
    }
    // Different call arities do not distinguish these opaque helpers: the
    // encoding mixer must not resolve to the reseed factory, even via stable
    // aliases declared in their enclosing function.
    if captured_callable_root(model, outer_site, &first.callee)?
        == captured_callable_root(model, outer_site, &reseed.callee)?
    {
        return None;
    }
    let arguments = mixer_arguments(model, cursor_value, mixer, random, bytes)?;
    if !is_symbol(model, arguments[1], count) || !number(arguments[3], 3.0) {
        return None;
    }
    mixer_loop(
        model,
        encode,
        array,
        count,
        forward_index,
        cursor,
        mixer,
        random,
        bytes,
    )?;
    let emit = call(statement_expression(emit)?)?;
    if slot_of(model, member(&emit.callee, "next")?, outer_site) != Some(0) {
        return None;
    }
    let arguments = exact_arguments(emit, 1)?;
    let payload = array_elements(arguments[0].without_parentheses())?;
    let [group, result] = payload.as_slice() else {
        return None;
    };
    if !empty_array(group) || !is_symbol(model, result, bytes) {
        return None;
    }
    let Statement::ReturnStatement(exit) = exit else {
        return None;
    };
    let Expression::UnaryExpression(void) = exit.argument.as_ref()?.without_parentheses() else {
        return None;
    };
    (void.operator.as_str() == "void" && number(&void.argument, 0.0)).then_some(())
}

fn seeded_bytes(
    model: &Model<'_, '_>,
    arrow: &ArrowFunctionExpression<'_>,
    entries: &[(Symbol, &Expression<'_>)],
    outer_site: FunctionSite,
    array: Symbol,
    pixels: Symbol,
    data: Symbol,
) -> bool {
    // Inspect the actual sampler statements. A nested or sibling donor is
    // never traversed, and a valid catch cannot hide a broken success arm.
    let mut tries = arrow.body.statements.iter().filter_map(|statement| {
        if let Statement::TryStatement(found) = statement {
            Some(found)
        } else {
            None
        }
    });
    let Some(found) = tries.next() else {
        return false;
    };
    let Some(handler) = found.handler.as_ref() else {
        return false;
    };
    tries.next().is_none()
        && found.finalizer.is_none()
        && seeded_arm(model, &found.block.body, outer_site, array, pixels, entries).is_some()
        && seeded_arm(
            model,
            &handler.body.body,
            outer_site,
            array,
            pixels,
            entries,
        )
        .is_some()
        && reaching_pixel_contents(model, found, entries, pixels, data).is_some()
}

/// A closed set of vector uses proves contents, not just a historical binding
/// identity. Unsupported reads/escapes are conservatively refused too. This is
/// deliberately a small protocol reader, not a JavaScript effect interpreter.
fn reaching_pixel_contents(
    model: &Model<'_, '_>,
    found: &TryStatement<'_>,
    entries: &[(Symbol, &Expression<'_>)],
    pixels: Symbol,
    data: Symbol,
) -> Option<()> {
    let success = &found.block.body;
    // The validated tail proves each append and complete mixer.
    // Its immediate predecessor must be the actual sampling loop, with no
    // intervening statement able to replace or clear the row.
    let tail = seeded_tail(success)?;
    let Statement::ForStatement(sample) =
        success.get(success.len().checked_sub(tail.len() + 1)?)?
    else {
        return None;
    };
    let [read, push] = block(&sample.body)? else {
        return None;
    };
    let (read_symbol, _) = local_value(read)?;
    if read_symbol != data {
        return None;
    }
    let push = call(statement_expression(push)?)?;
    let receiver = member(&push.callee, "push")?;
    let args = exact_arguments(push, 1)?;
    if root_symbol(model, receiver) != Some(pixels)
        || rgba_lanes(model, args[0], data) != Some(0b1111)
    {
        return None;
    }
    let mut allowed = vec![receiver.without_parentheses().span()];
    for arm in [
        success.as_slice(),
        found.handler.as_ref()?.body.body.as_slice(),
    ] {
        let append = call(statement_expression(seeded_tail(arm)?.first()?)?)?;
        let args = call_arguments(append);
        allowed.push(args.last()?.without_parentheses().span());
    }
    // Follow simple aliases even if later overwritten, so unstable aliases
    // cannot hide a write or escape from this reference inventory.
    let mut aliases = vec![pixels];
    loop {
        let before = aliases.len();
        for (alias, value) in entries {
            if symbol(model, value).is_some_and(|source| aliases.contains(&source)) {
                if !single_origin(model, *alias) || root_symbol(model, value) != Some(pixels) {
                    return None;
                }
                allowed.push(value.without_parentheses().span());
                if !aliases.contains(alias) {
                    aliases.push(*alias);
                }
            }
        }
        if aliases.len() == before {
            break;
        }
    }
    // All resolved uses count, including nested closures. Only exact operand
    // spans above are authorized: member writes, deletes, updates, method
    // calls, argument escapes and donor replacements have no permitted use.
    for alias in aliases {
        for reference in model.scoping.get_resolved_references(alias) {
            let span = model.nodes.get_node(reference.node_id()).span();
            if !allowed.contains(&span) {
                return None;
            }
        }
    }
    Some(())
}

fn prove_sampler<'a>(
    model: &Model<'_, 'a>,
    outer_site: FunctionSite,
    sampler_site: FunctionSite,
    arrow: &'a ArrowFunctionExpression<'a>,
) -> Option<Sampler> {
    if arrow.r#async || arrow.params.rest.is_some() {
        return None;
    }
    let [parameter] = arrow.params.items.as_slice() else {
        return None;
    };
    if parameter.initializer.is_some() {
        return None;
    }
    let message = binding_symbol(&parameter.pattern)?;
    if !arrow.body.directives.is_empty() {
        return None;
    }
    let start = arrow.body.span.start as usize;
    let end = arrow.body.span.end as usize;
    let body_source = &model.source[start..end];
    // Cheap prefilter only: the structural proof below still requires the
    // owned context, text draw, same-receiver 1x1 pixel read and seeded sink.
    if !body_source.contains("getContext")
        || !body_source.contains("fillText")
        || !body_source.contains("getImageData")
    {
        return None;
    }
    let scan = scan_sampler(model, sampler_site);
    let entries = values(model, &scan);

    let canvas = canvas_size(model, &scan)?;
    let context = canvas_context(model, &entries, canvas)?;
    if context == canvas || context == message {
        return None;
    }
    if !draws_text(model, &scan, &entries, context, message) {
        return None;
    }
    let data = pixel_data(model, &entries, context, message)?;
    let pixels = pixel_vector(model, &scan, data)?;
    if pixels == canvas || pixels == context || pixels == message || pixels == data {
        return None;
    }
    let array = accumulator(model, &scan, &entries, pixels)?;
    if array == canvas || array == context || array == message || array == data || array == pixels {
        return None;
    }
    if !seeded_bytes(model, arrow, &entries, outer_site, array, pixels, data) {
        return None;
    }
    // The font and fill protocol are part of the glyph sampler, not a donor.
    let mut font = false;
    let mut fill = false;
    for assignment in &scan.assignments {
        let AssignmentTarget::StaticMemberExpression(found) = &assignment.left else {
            continue;
        };
        if found.optional || root_symbol(model, &found.object) != Some(context) {
            continue;
        }
        match found.property.name.as_str() {
            "font" => font = font_value(model, &assignment.right, canvas),
            "fillStyle" => fill = string_value(&assignment.right) == Some("#000"),
            _ => {}
        }
    }
    if !font || !fill {
        return None;
    }
    Some(Sampler {
        canvas,
        array,
        message,
        context,
        pixels,
    })
}

// ---------------------------------------------------------------------------
// Outer collector proof
// ---------------------------------------------------------------------------

fn canvas_created(model: &Model<'_, '_>, site: FunctionSite, canvas: Symbol) -> bool {
    if !single_origin(model, canvas) {
        return false;
    }
    let Some(created) = model.symbol_value(canvas) else {
        return false;
    };
    let Some(created) = call(created) else {
        return false;
    };
    let Some(receiver) = member(&created.callee, "createElement") else {
        return false;
    };
    slot_of(model, receiver, site) == Some(1)
        && call_arguments(created)
            .first()
            .is_some_and(|argument| string_value(argument) == Some("canvas"))
}

fn array_created(model: &Model<'_, '_>, array: Symbol) -> bool {
    single_origin(model, array) && model.symbol_value(array).is_some_and(empty_array)
}

fn started(model: &Model<'_, '_>, calls: &[&CallExpression<'_>], queue: Symbol) -> bool {
    calls.iter().any(|found| {
        member(&found.callee, "start")
            .is_some_and(|receiver| root_symbol(model, receiver) == Some(queue))
    })
}

fn on_empty(
    model: &Model<'_, '_>,
    calls: &[&CallExpression<'_>],
    queue: Symbol,
) -> Option<FunctionSite> {
    for found in calls {
        if let Some(receiver) = member(&found.callee, "onEmpty") {
            if root_symbol(model, receiver) == Some(queue) {
                return call_arguments(found)
                    .into_iter()
                    .find_map(|argument| model.function_from_expression(argument));
            }
        }
        let Some((holder, index, arguments)) = holder_method_call(model, found) else {
            continue;
        };
        if index != 1 || arguments.len() < 2 || !holder_index(model, arguments[0], holder, 0) {
            continue;
        }
        if !single_origin(model, holder) {
            continue;
        }
        let Some(value) = model.symbol_value(holder) else {
            continue;
        };
        if !defined_before(value, found.span.start) {
            continue;
        }
        let Some(elements) = array_elements(value.without_parentheses()) else {
            continue;
        };
        let [context_argument, method] = elements.as_slice() else {
            continue;
        };
        if root_symbol(model, context_argument) != Some(queue) {
            continue;
        }
        if member(method, "onEmpty")
            .is_none_or(|receiver| root_symbol(model, receiver) != Some(queue))
        {
            continue;
        }
        return model.function_from_expression(arguments[1]);
    }
    None
}

fn sink_consumer(
    model: &Model<'_, '_>,
    outer_site: FunctionSite,
    callback: FunctionSite,
    array: Symbol,
) -> bool {
    let AstKind::ArrowFunctionExpression(arrow) = model.nodes.kind(callback.node_id) else {
        return false;
    };
    if !arrow.params.items.is_empty() || arrow.params.rest.is_some() {
        return false;
    }
    if !arrow.body.directives.is_empty() {
        return false;
    }
    let [Statement::ExpressionStatement(statement)] = arrow.body.statements.as_slice() else {
        return false;
    };
    let Some(consumer) = call(&statement.expression) else {
        return false;
    };
    if slot_of(model, &consumer.callee, outer_site) != Some(3) {
        return false;
    }
    let arguments = call_arguments(consumer);
    let [value] = arguments.as_slice() else {
        return false;
    };
    root_symbol(model, value) == Some(array)
}

fn teardown(body: &FunctionBody<'_>, queue: Symbol, model: &Model<'_, '_>) -> Option<Symbol> {
    let mut returned = None;
    for statement in &body.statements {
        if let Statement::ReturnStatement(returned_statement) = statement {
            if let Some(argument) = returned_statement.argument.as_ref() {
                returned = symbol(model, argument);
            }
        }
    }
    let returned = returned?;
    for statement in &body.statements {
        let Statement::FunctionDeclaration(function) = statement else {
            continue;
        };
        let Some(identifier) = function.id.as_ref().and_then(|id| id.symbol_id.get()) else {
            continue;
        };
        if identifier != returned {
            continue;
        }
        let Some(function_body) = function.body.as_ref() else {
            continue;
        };
        if function.r#async
            || function.generator
            || function.params.rest.is_some()
            || !function.params.items.is_empty()
            || !function_body.directives.is_empty()
        {
            continue;
        }
        let [Statement::IfStatement(guard)] = function_body.statements.as_slice() else {
            continue;
        };
        if guard.alternate.is_some() || root_symbol(model, &guard.test) != Some(queue) {
            continue;
        }
        let Some(consequent) = block(&guard.consequent) else {
            continue;
        };
        let [Statement::ExpressionStatement(statement)] = consequent else {
            continue;
        };
        let Some(stop) = call(&statement.expression) else {
            continue;
        };
        if member(&stop.callee, "empty")
            .is_some_and(|receiver| root_symbol(model, receiver) == Some(queue))
        {
            return Some(returned);
        }
    }
    None
}

fn prove(
    model: &Model<'_, '_>,
    site: FunctionSite,
    facts: &Facts<'_>,
    function: &Function<'_>,
) -> Option<()> {
    if function.r#async || function.generator {
        return None;
    }
    let body = function.body.as_ref()?;
    if !body.directives.is_empty() {
        return None;
    }
    let formals = plain_formals(model, function, 4)?;

    let mut found = None;
    for found_call in &facts.calls {
        let arguments = call_arguments(found_call);
        for (index, argument) in arguments.iter().enumerate() {
            let Some(candidate) = model.function_from_expression(argument) else {
                continue;
            };
            if candidate.span == site.span || !body.span.contains_inclusive(candidate.span) {
                continue;
            }
            let AstKind::ArrowFunctionExpression(arrow) = model.nodes.kind(candidate.node_id)
            else {
                continue;
            };
            let Some(factory) = symbol(model, &found_call.callee) else {
                continue;
            };
            if !captured(model, site, factory) {
                continue;
            }
            if !arguments
                .iter()
                .skip(index + 1)
                .any(|argument| slot_of(model, argument, site) == Some(2))
            {
                continue;
            }
            let Some(queue) = call_binding(model, found_call) else {
                continue;
            };
            let Some(sampler) = prove_sampler(model, site, candidate, arrow) else {
                continue;
            };
            found = Some((queue, sampler));
            break;
        }
        if found.is_some() {
            break;
        }
    }
    let (queue, sampler) = found?;
    if formals.contains(&sampler.canvas)
        || formals.contains(&sampler.array)
        || formals.contains(&queue)
    {
        return None;
    }
    if !body
        .span
        .contains_inclusive(model.scoping.symbol_span(sampler.canvas))
        || !body
            .span
            .contains_inclusive(model.scoping.symbol_span(sampler.array))
        || !body
            .span
            .contains_inclusive(model.scoping.symbol_span(queue))
    {
        return None;
    }
    if !canvas_created(model, site, sampler.canvas) || !array_created(model, sampler.array) {
        return None;
    }
    // The canvas, accumulator and queue are the collector's own participants:
    // their one definition must be owned by this outer body, not by the nested
    // sampler. A sampler-private canvas is refused even when it is a
    // document.createElement("canvas").
    if !defined_in(model, site, sampler.canvas)
        || !defined_in(model, site, sampler.array)
        || !single_origin(model, queue)
        || !defined_in(model, site, queue)
    {
        return None;
    }
    if !started(model, &facts.calls, queue) {
        return None;
    }
    let callback = on_empty(model, &facts.calls, queue)?;
    if !sink_consumer(model, site, callback, sampler.array) {
        return None;
    }
    teardown(body, queue, model)?;
    Some(())
}

pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    facts: &Facts<'_>,
    _proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return Vec::new();
    };
    if prove(model, site, facts, function).is_none() {
        return Vec::new();
    }
    vec![proposal(
        ROLE,
        EVALUATOR,
        "owned 96x96 canvas text collector: creates the canvas through the captured document slot, schedules the captured queue factory with the nested glyph sampler, proves the sampler draws the caller glyph with fillText on a willReadFrequently 2d context of the same canvas, reads every caller point with getImageData(..., 1, 1).data from that same context, pins the packed RGBA lanes, pushes the pixel vector into the outer accumulator, derives and reseeds the Math.random 32-bit word with a captured hash, folds the same accumulator through a captured mixer with the same seed/header and threaded cursor in both success and catch, emits [[], bytes] on the captured observer slot, registers the sink slot as the queue onEmpty consumer, starts the queue and returns a teardown that empties the same queue",
        facts,
    )]
}
