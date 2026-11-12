//! Bounded owned-body proof for the final byte-payload assembly of a signal
//! snapshot.
//!
//! The recognized body takes one snapshot record and turns it into the final
//! byte array (or the checksum-prefixed result returned by the caller):
//!
//! ```text
//! function encode(snapshot) {
//!   let out = [];
//!   let errors = snapshot.errors.length === 0 ? [] : encodeValue(snapshot.errors);
//!   // seven 7-bit varint frames: errors, positional key/value, signals,
//!   // keyed key/number/value
//!   if (errors.length >>> 0 < 128) { ... } else { ... >>> 7 ... & 127 ... }
//!   ...
//!   for (; i < snapshot.positionalSignals.length; i++) {
//!     if (snapshot.positionalSignals[i] === sentinel) { out.push(0); out.push(0); }
//!     else { ... encodeValue(snapshot.positionalSignals[i][0]) ... }
//!   }
//!   const named = snapshot.signals.length === 0 ? [] : encodeValue(snapshot.signals, options);
//!   ...
//!   while (k < snapshot.keyedSignals.length) {
//!     if (snapshot.keyedSignals[k][1] === sentinel) { out.push(0); out.push(0); }
//!     else { ... encodeValue(snapshot.keyedSignals[k][1][0]) ... }
//!   }
//!   if (hasher != null && key != null) {
//!     ... Math.random / performance.now / new Date / crypto.getRandomValues ...
//!     out = hasher(out, key, seedA, seedB);
//!     out.unshift(seedA >>> 24, ..., seedB & 255);
//!   }
//!   return out; // or a local that receives the checked/aliased byte array
//! }
//! ```
//!
//! Every identity is scoped Oxc symbol equality. The snapshot parameter must be
//! read directly and only through `errors`, `positionalSignals`, `signals` and
//! `keyedSignals`; the sentinel, value encoder, key and hasher are captured
//! bindings declared outside the owned body; the payload array is a local empty
//! array receiving the pushes. Local single-origin aliases are followed for the
//! member chain and for the returned result, which covers the `let row =
//! snapshot.keyedSignals[i]` spelling seen in one vendor. No source spelling,
//! generated thread id, KEY/name dictionary, source offset, vendor or hash gate
//! participates, and nested functions are never descended into (their operations
//! are not owned by this body).
//!
//! Deliberately partial: this proves the byte-assembly variant. The older
//! `function_finder::signal_bundle_encoder` rule owns the base64-text variant
//! with its `charAt` emission, so this fallback only runs after that and every
//! other earlier stage has declined.

use super::*;
use oxc_semantic::SymbolId;
use oxc_span::GetSpan;

type Symbol = SymbolId;

const FIELDS: [&str; 4] = ["errors", "positionalSignals", "signals", "keyedSignals"];

fn field_name(name: &str) -> Option<&'static str> {
    FIELDS.iter().copied().find(|field| *field == name)
}

fn symbol(model: &Model<'_, '_>, expression: &Expression<'_>) -> Option<Symbol> {
    crate::analysis::expression_symbol(expression.without_parentheses(), model.scoping)
}

/// A plain symbol declared inside the owned function.
fn local_symbol(model: &Model<'_, '_>, site: FunctionSite, symbol: Symbol) -> bool {
    site.span
        .contains_inclusive(model.scoping.symbol_span(symbol))
        && model.scoping.symbol_redeclarations(symbol).is_empty()
}

/// A variable declarator binding declared outside the owned function. The
/// sentinel, value encoder, key and hasher are all captured provider values.
fn captured(
    model: &Model<'_, '_>,
    site: FunctionSite,
    expression: &Expression<'_>,
) -> Option<Symbol> {
    let captured = symbol(model, expression)?;
    if site
        .span
        .contains_inclusive(model.scoping.symbol_span(captured))
        || !model.scoping.symbol_redeclarations(captured).is_empty()
    {
        return None;
    }
    let AstKind::VariableDeclarator(variable) =
        model.nodes.kind(model.scoping.symbol_declaration(captured))
    else {
        return None;
    };
    matches!(&variable.id, BindingPattern::BindingIdentifier(id)
            if id.symbol_id.get() == Some(captured))
    .then_some(captured)
}

fn member<'a>(expression: &'a Expression<'a>, key: &str) -> Option<&'a Expression<'a>> {
    let Expression::StaticMemberExpression(member) = expression.without_parentheses() else {
        return None;
    };
    (!member.optional && member.property.name == key).then_some(&member.object)
}

fn new_arguments<'a>(new: &'a NewExpression<'a>) -> Vec<&'a Expression<'a>> {
    new.arguments
        .iter()
        .filter_map(Argument::as_expression)
        .collect()
}

fn numeric(expression: &Expression<'_>, value: f64) -> bool {
    matches!(expression.without_parentheses(), Expression::NumericLiteral(number) if number.value == value)
}

/// The field read directly off the snapshot parameter: `snapshot.errors` or
/// `snapshot["positionalSignals"]`, never through a deeper member.
fn direct_field<'a>(
    model: &Model<'_, '_>,
    expression: &'a Expression<'a>,
    parameter: Symbol,
) -> Option<&'static str> {
    let expression = expression.without_parentheses();
    match expression {
        Expression::StaticMemberExpression(member) => (symbol(model, &member.object)
            == Some(parameter))
        .then(|| field_name(member.property.name.as_str()))
        .flatten(),
        Expression::ComputedMemberExpression(member) => (symbol(model, &member.object)
            == Some(parameter))
        .then(|| match &member.expression {
            Expression::StringLiteral(literal) => field_name(literal.value.as_str()),
            _ => None,
        })
        .flatten(),
        _ => None,
    }
}

/// A local whose only source origin is one plain value: a `let x = value`
/// initializer or one later `x = value` assignment.
fn local_value<'a>(
    model: &Model<'_, 'a>,
    site: FunctionSite,
    symbol: Symbol,
) -> Option<&'a Expression<'a>> {
    if !local_symbol(model, site, symbol) {
        return None;
    }
    let declaration = model.scoping.symbol_declaration(symbol);
    let AstKind::VariableDeclarator(variable) = model.nodes.kind(declaration) else {
        return None;
    };
    let writes = model.binding_writes(symbol);
    if !(writes == 0 && variable.init.is_some() || writes == 1 && variable.init.is_none()) {
        return None;
    }
    model.symbol_value(symbol)
}

/// The snapshot field a member chain reads, following single-origin local
/// aliases. `snapshot.signals` and `const row = snapshot.keyedSignals[i]`
/// followed by `row[1][0]` both resolve to their field.
fn field_of(
    model: &Model<'_, '_>,
    site: FunctionSite,
    parameter: Symbol,
    expression: &Expression<'_>,
    depth: usize,
) -> Option<&'static str> {
    if depth > 6 {
        return None;
    }
    let expression = expression.without_parentheses();
    if let Some(field) = direct_field(model, expression, parameter) {
        return Some(field);
    }
    match expression {
        Expression::StaticMemberExpression(member) => {
            field_of(model, site, parameter, &member.object, depth + 1)
        }
        Expression::ComputedMemberExpression(member) => {
            field_of(model, site, parameter, &member.object, depth + 1)
        }
        Expression::Identifier(identifier) => {
            let alias = crate::analysis::identifier_symbol(identifier, model.scoping)?;
            let value = local_value(model, site, alias)?;
            field_of(model, site, parameter, value, depth + 1)
        }
        _ => None,
    }
}

/// Read-only census of the owned body. Nested functions are counted and never
/// descended, so a decoy nested callback cannot supply the operations.
#[derive(Default)]
struct Ops<'a> {
    nested: usize,
    calls: Vec<&'a CallExpression<'a>>,
    news: Vec<&'a NewExpression<'a>>,
    binaries: Vec<&'a BinaryExpression<'a>>,
    assignments: Vec<&'a AssignmentExpression<'a>>,
    updates: Vec<&'a UpdateExpression<'a>>,
    members: Vec<&'a Expression<'a>>,
    declarations: Vec<&'a VariableDeclarator<'a>>,
    returns: Vec<&'a Expression<'a>>,
    conditionals: Vec<&'a IfStatement<'a>>,
    loops: usize,
}

impl<'a> Ops<'a> {
    fn body(&mut self, body: &'a FunctionBody<'a>) {
        for statement in &body.statements {
            self.statement(statement);
        }
    }

    fn statements(&mut self, statements: &'a [Statement<'a>]) {
        for statement in statements {
            self.statement(statement);
        }
    }

    fn variable_declaration(&mut self, declaration: &'a VariableDeclaration<'a>) {
        for declarator in &declaration.declarations {
            self.declarations.push(declarator);
            if let Some(init) = &declarator.init {
                self.expression(init);
            }
        }
    }

    fn statement(&mut self, statement: &'a Statement<'a>) {
        match statement {
            Statement::VariableDeclaration(declaration) => self.variable_declaration(declaration),
            Statement::ExpressionStatement(statement) => self.expression(&statement.expression),
            Statement::ReturnStatement(statement) => {
                if let Some(argument) = &statement.argument {
                    self.returns.push(argument);
                    self.expression(argument);
                }
            }
            Statement::BlockStatement(block) => self.statements(&block.body),
            Statement::IfStatement(branch) => {
                self.conditionals.push(branch);
                self.expression(&branch.test);
                self.statement(&branch.consequent);
                if let Some(alternate) = &branch.alternate {
                    self.statement(alternate);
                }
            }
            Statement::ForStatement(loop_) => {
                self.loops += 1;
                if let Some(ForStatementInit::VariableDeclaration(declaration)) = &loop_.init {
                    self.variable_declaration(declaration);
                }
                if let Some(test) = &loop_.test {
                    self.expression(test);
                }
                if let Some(update) = &loop_.update {
                    self.expression(update);
                }
                self.statement(&loop_.body);
            }
            Statement::ForInStatement(loop_) => {
                self.loops += 1;
                self.expression(&loop_.right);
                self.statement(&loop_.body);
            }
            Statement::ForOfStatement(loop_) => {
                self.loops += 1;
                self.expression(&loop_.right);
                self.statement(&loop_.body);
            }
            Statement::WhileStatement(loop_) => {
                self.loops += 1;
                self.expression(&loop_.test);
                self.statement(&loop_.body);
            }
            Statement::DoWhileStatement(loop_) => {
                self.loops += 1;
                self.statement(&loop_.body);
                self.expression(&loop_.test);
            }
            Statement::TryStatement(attempt) => {
                self.statements(&attempt.block.body);
                if let Some(handler) = &attempt.handler {
                    self.statements(&handler.body.body);
                }
                if let Some(finalizer) = &attempt.finalizer {
                    self.statements(&finalizer.body);
                }
            }
            Statement::SwitchStatement(switch) => {
                self.expression(&switch.discriminant);
                for case in &switch.cases {
                    if let Some(test) = &case.test {
                        self.expression(test);
                    }
                    self.statements(&case.consequent);
                }
            }
            Statement::ThrowStatement(thrown) => self.expression(&thrown.argument),
            Statement::LabeledStatement(labeled) => self.statement(&labeled.body),
            Statement::WithStatement(with) => {
                self.expression(&with.object);
                self.statement(&with.body);
            }
            Statement::FunctionDeclaration(_) | Statement::ClassDeclaration(_) => self.nested += 1,
            _ => {}
        }
    }

    fn expression(&mut self, expression: &'a Expression<'a>) {
        match expression.without_parentheses() {
            Expression::CallExpression(call) => {
                self.calls.push(call);
                self.expression(&call.callee);
                for argument in call_arguments(call) {
                    self.expression(argument);
                }
            }
            Expression::NewExpression(new) => {
                self.news.push(new);
                self.expression(&new.callee);
                for argument in new_arguments(new) {
                    self.expression(argument);
                }
            }
            Expression::BinaryExpression(binary) => {
                self.binaries.push(binary);
                self.expression(&binary.left);
                self.expression(&binary.right);
            }
            Expression::LogicalExpression(logical) => {
                self.expression(&logical.left);
                self.expression(&logical.right);
            }
            Expression::UnaryExpression(unary) => self.expression(&unary.argument),
            Expression::UpdateExpression(update) => {
                self.updates.push(update);
            }
            Expression::AssignmentExpression(assignment) => {
                self.assignments.push(assignment);
                self.expression(&assignment.right);
            }
            Expression::ConditionalExpression(conditional) => {
                self.expression(&conditional.test);
                self.expression(&conditional.consequent);
                self.expression(&conditional.alternate);
            }
            Expression::StaticMemberExpression(member) => {
                self.members.push(expression);
                self.expression(&member.object);
            }
            Expression::ComputedMemberExpression(member) => {
                self.members.push(expression);
                self.expression(&member.object);
                self.expression(&member.expression);
            }
            Expression::ArrayExpression(array) => {
                for element in &array.elements {
                    if let Some(expression) = element.as_expression() {
                        self.expression(expression);
                    }
                }
            }
            Expression::SequenceExpression(sequence) => {
                for expression in &sequence.expressions {
                    self.expression(expression);
                }
            }
            Expression::TemplateLiteral(template) => {
                for expression in &template.expressions {
                    self.expression(expression);
                }
            }
            Expression::TaggedTemplateExpression(tagged) => {
                self.expression(&tagged.tag);
                for expression in &tagged.quasi.expressions {
                    self.expression(expression);
                }
            }
            Expression::AwaitExpression(await_) => self.expression(&await_.argument),
            Expression::YieldExpression(yield_) => {
                if let Some(argument) = &yield_.argument {
                    self.expression(argument);
                }
            }
            Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_) => {
                self.nested += 1
            }
            _ => {}
        }
    }
}

fn call_on<'a>(call: &'a CallExpression<'a>, name: &str) -> Option<&'a Expression<'a>> {
    member(&call.callee, name)
}

fn pushes_to(model: &Model<'_, '_>, call: &CallExpression<'_>) -> Option<Symbol> {
    symbol(model, call_on(call, "push")?)
}

fn unshift_of<'a>(call: &'a CallExpression<'a>) -> Option<&'a Expression<'a>> {
    call_on(call, "unshift")
}

fn empty_array(expression: &Expression<'_>) -> bool {
    matches!(expression.without_parentheses(), Expression::ArrayExpression(array) if array.elements.is_empty())
}

/// The single local array that receives the assembly pushes and starts as `[]`.
fn payload_array(model: &Model<'_, '_>, site: FunctionSite, ops: &Ops<'_>) -> Option<Symbol> {
    let mut counts = BTreeMap::<Symbol, usize>::new();
    for call in &ops.calls {
        if let Some(receiver) = pushes_to(model, call) {
            *counts.entry(receiver).or_default() += 1;
        }
    }
    let mut candidates = Vec::new();
    for (symbol, count) in counts {
        if count < 4 || !local_symbol(model, site, symbol) {
            continue;
        }
        let starts_empty = ops.declarations.iter().any(|variable| {
            matches!(&variable.id, BindingPattern::BindingIdentifier(id)
                if id.symbol_id.get() == Some(symbol))
                && variable.init.as_ref().is_some_and(empty_array)
        }) || ops.assignments.iter().any(|assignment| {
            matches!(&assignment.left, AssignmentTarget::AssignmentTargetIdentifier(id)
                if crate::analysis::identifier_symbol(id, model.scoping) == Some(symbol))
                && empty_array(&assignment.right)
        });
        if starts_empty {
            candidates.push(symbol);
        }
    }
    match candidates.as_slice() {
        [payload] => Some(*payload),
        _ => None,
    }
}

/// `count` zero literals pushed into `payload` inside `span`.
fn zero_pushes(
    model: &Model<'_, '_>,
    ops: &Ops<'_>,
    payload: Symbol,
    span: oxc_span::Span,
) -> usize {
    ops.calls
        .iter()
        .filter(|call| {
            call.span.start >= span.start
                && call.span.end <= span.end
                && pushes_to(model, call) == Some(payload)
                && matches!(call_arguments(call).as_slice(), [argument] if numeric(argument, 0.0))
        })
        .count()
}

fn equality_field(
    model: &Model<'_, '_>,
    site: FunctionSite,
    ops: &Ops<'_>,
    parameter: Symbol,
    field: &'static str,
) -> Option<(Symbol, oxc_span::Span)> {
    for branch in &ops.conditionals {
        let Expression::BinaryExpression(binary) = branch.test.without_parentheses() else {
            continue;
        };
        if binary.operator.as_str() != "===" {
            continue;
        }
        for (side, other) in [(&binary.left, &binary.right), (&binary.right, &binary.left)] {
            if field_of(model, site, parameter, side, 0) != Some(field) {
                continue;
            }
            if let Some(sentinel) = captured(model, site, other) {
                return Some((sentinel, branch.consequent.span()));
            }
        }
    }
    None
}

fn varint_frames(model: &Model<'_, '_>, ops: &Ops<'_>) -> usize {
    ops.conditionals
        .iter()
        .filter(|branch| {
            let Expression::BinaryExpression(binary) = branch.test.without_parentheses() else {
                return false;
            };
            binary.operator.as_str() == "<"
                && numeric(&binary.right, 128.0)
                && unsigned_coercion(model, &binary.left)
        })
        .count()
}

fn unsigned_coercion(_model: &Model<'_, '_>, expression: &Expression<'_>) -> bool {
    let mut found = false;
    for binary in model_binaries(expression) {
        found |= binary.operator.as_str() == ">>>" && numeric(&binary.right, 0.0);
    }
    // A `.length` read is the other framing subject.
    if member(expression, "length").is_some() {
        found = true;
    }
    found
}

/// Binaries inside an expression, without descending into nested functions.
fn model_binaries<'a>(expression: &'a Expression<'a>) -> Vec<&'a BinaryExpression<'a>> {
    let mut ops = Ops::default();
    ops.expression(expression);
    ops.binaries
}

fn path_is(model: &Model<'_, '_>, expression: &Expression<'_>, expected: &[&str]) -> bool {
    let path = expression_path(model, expression);
    path.len() == expected.len()
        && path
            .iter()
            .zip(expected)
            .all(|(actual, expected)| actual == expected)
}

fn global_call(
    model: &Model<'_, '_>,
    call: &CallExpression<'_>,
    path: &[&str],
    name: &str,
) -> bool {
    let Some(object) = member(&call.callee, name) else {
        return false;
    };
    path_is(model, object, path)
}

fn byte_encoder(
    model: &Model<'_, '_>,
    site: FunctionSite,
    parameter: Symbol,
    ops: &Ops<'_>,
) -> Option<Symbol> {
    let mut encoder: Option<Symbol> = None;
    let mut seen = BTreeSet::new();
    for call in &ops.calls {
        let Some(callee) = captured(model, site, &call.callee) else {
            continue;
        };
        let arguments = call_arguments(call);
        if arguments.is_empty() || arguments.len() > 2 {
            continue;
        }
        let fields: Vec<_> = arguments
            .iter()
            .filter_map(|argument| field_of(model, site, parameter, argument, 0))
            .collect();
        if fields.is_empty() {
            continue;
        }
        match encoder {
            Some(existing) if existing != callee => return None,
            _ => encoder = Some(callee),
        }
        seen.extend(fields);
    }
    let encoder = encoder?;
    (seen.len() == FIELDS.len()).then_some(encoder)
}

fn entropy(
    model: &Model<'_, '_>,
    site: FunctionSite,
    payload: Symbol,
    returned: Symbol,
    ops: &Ops<'_>,
) -> Option<()> {
    let mut random = 0;
    let mut clock = 0;
    let mut entropy = 0;
    for call in &ops.calls {
        if global_call(model, call, &["Math"], "random") {
            random += 1;
        }
        if global_call(model, call, &["performance"], "now") {
            clock += 1;
        }
        if global_call(model, call, &["crypto"], "getRandomValues") {
            entropy += 1;
        }
    }
    let date = ops
        .news
        .iter()
        .any(|new| path_is(model, &new.callee, &["Date"]) && new.arguments.is_empty());
    let typed = ops.news.iter().any(|new| {
        path_is(model, &new.callee, &["Uint32Array"])
            && matches!(new_arguments(new).as_slice(), [argument] if numeric(argument, 1.0))
    });
    let xors = ops
        .binaries
        .iter()
        .filter(|binary| binary.operator.as_str() == "^")
        .count();
    let bytes = ops
        .binaries
        .iter()
        .filter(|binary| binary.operator.as_str() == "&" && numeric(&binary.right, 255.0))
        .count();
    let high_bytes = ops
        .binaries
        .iter()
        .filter(|binary| binary.operator.as_str() == ">>>" && numeric(&binary.right, 24.0))
        .count();
    let hasher = ops.assignments.iter().any(|assignment| {
        let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else {
            return false;
        };
        crate::analysis::identifier_symbol(target, model.scoping) == Some(returned)
            && matches!(assignment.right.without_parentheses(), Expression::CallExpression(call)
                if call.arguments.len() == 4
                    && captured(model, site, &call.callee).is_some()
                    && call_arguments(call).first().and_then(|argument| symbol(model, argument)) == Some(payload))
    });
    let unshift = ops.calls.iter().any(|call| {
        unshift_of(call).and_then(|object| symbol(model, object)) == Some(returned)
            && call_arguments(call).len() == 8
    });
    if random == 0
        || clock == 0
        || entropy == 0
        || !date
        || !typed
        || xors < 6
        || bytes < 4
        || high_bytes < 2
        || !hasher
        || !unshift
    {
        return None;
    }
    Some(())
}

pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    f: &Facts<'_>,
    _proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return Vec::new();
    };
    let Some(body) = &function.body else {
        return Vec::new();
    };
    if function.r#async
        || function.generator
        || function.params.rest.is_some()
        || !body.directives.is_empty()
    {
        return Vec::new();
    }
    let [formal] = function.params.items.as_slice() else {
        return Vec::new();
    };
    if formal.initializer.is_some() {
        return Vec::new();
    }
    let BindingPattern::BindingIdentifier(identifier) = &formal.pattern else {
        return Vec::new();
    };
    let Some(parameter) = identifier.symbol_id.get() else {
        return Vec::new();
    };
    if model.binding_writes(parameter) != 0
        || !site
            .span
            .contains_inclusive(model.scoping.symbol_span(parameter))
    {
        return Vec::new();
    }
    let proved = (|| -> Option<()> {
        let mut ops = Ops::default();
        ops.body(body);
        if ops.nested != 0 || ops.returns.len() != 1 {
            return None;
        }
        // Only the four snapshot fields may be read directly off the parameter.
        let mut fields = BTreeSet::new();
        for member in &ops.members {
            let direct = match member.without_parentheses() {
                Expression::StaticMemberExpression(static_member)
                    if symbol(model, &static_member.object) == Some(parameter) =>
                {
                    Some(static_member.property.name.as_str())
                }
                Expression::ComputedMemberExpression(computed)
                    if symbol(model, &computed.object) == Some(parameter) =>
                {
                    match &computed.expression {
                        Expression::StringLiteral(literal) => Some(literal.value.as_str()),
                        _ => None,
                    }
                }
                _ => None,
            };
            if let Some(name) = direct {
                fields.insert(field_name(name)?);
            }
        }
        if fields.len() != FIELDS.len() {
            return None;
        }
        let encoder = byte_encoder(model, site, parameter, &ops)?;
        let payload = payload_array(model, site, &ops)?;
        let returned = symbol(model, ops.returns[0])?;
        if !local_symbol(model, site, returned)
            || returned == parameter
            || payload == parameter
            || returned == encoder
            || payload == encoder
        {
            return None;
        }
        let (positional_sentinel, positional_span) =
            equality_field(model, site, &ops, parameter, "positionalSignals")?;
        let (keyed_sentinel, keyed_span) =
            equality_field(model, site, &ops, parameter, "keyedSignals")?;
        if positional_sentinel != keyed_sentinel
            || positional_sentinel == parameter
            || positional_sentinel == encoder
            || positional_sentinel == payload
        {
            return None;
        }
        if zero_pushes(model, &ops, payload, positional_span) < 2
            || zero_pushes(model, &ops, payload, keyed_span) < 2
        {
            return None;
        }
        if ops
            .binaries
            .iter()
            .filter(|binary| binary.operator.as_str() == ">>>" && numeric(&binary.right, 7.0))
            .count()
            < 8
            || ops
                .binaries
                .iter()
                .filter(|binary| binary.operator.as_str() == "&" && numeric(&binary.right, 127.0))
                .count()
                < 6
            || varint_frames(model, &ops) < 6
        {
            return None;
        }
        entropy(model, site, payload, returned, &ops)
    })();
    if proved.is_none() {
        return Vec::new();
    }
    vec![proposal(
        "encodeSignalBundle",
        "shared_posthost_payload_finalizer",
        "one-formal owned body: reads only errors/positionalSignals/signals/keyedSignals off the snapshot; frames each array with seven 7-bit varint length frames (>>> 7 / & 127); encodes errors, positioned key, named signals and keyed key through one captured value encoder; two captured-sentinel branches push a two-zero marker; a guarded Math.random/performance.now/Date/crypto.getRandomValues entropy tail feeds a captured hasher over the byte array and prefixes the returned result through an eight-argument unshift; payload is the single local empty array receiving the assembly pushes and the returned local is the payload or its checked/aliased result; all sentinel/encoder/hasher identities are scoped captures and no nested callback is entered",
        f,
    )]
    .into_iter()
    .collect()
}
