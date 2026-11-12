//! Package `p07_request_envelope` finders: the URL parser and its cache, the
//! punycode branch, the route matcher and its wrapper, the request envelope
//! builder, the header-value chunker and the MAIN state they read. See
//! `roles/mod.rs` for the recognition rules.
//!
//! # How this package decides
//!
//! The anchors are host and public API names the program cannot rename
//! (`createElement("a")`, `decodeURIComponent`, `Object.keys`, `RegExp`,
//! `String.fromCharCode`), the URL field names of the parsed object and the
//! request (`protocol`, `pathname`, `url`, `method`), the stable option names
//! of the envelope builder (`signalFilter`, `encode`, `notifyLastChance`,
//! `descriptor`) and the anchor accessor names (`anchorHrefGet`,
//! `anchorHrefSet`). Every MAIN binding is proven through the position it
//! occupies inside a function this package has already proven, and through its
//! own single value where that value is meaningful.
//!
//! Both tracer shapes are read: a formal and `arguments[N]` are one slot, a
//! `name.method(...)` and `name["method"].call(name, ...)` are one call, and a
//! callback may be an inline arrow or a hoisted declaration.
//!
//! Each role here names one thing in a program, so a role that matches two
//! different spans is dropped whole in [`Claims::into_unique`]. The plan only
//! refuses two roles on one binding: it would give two bindings of one role
//! the same name, so that refusal has to happen here.

use std::collections::{BTreeMap, BTreeSet};

use oxc_ast::ast::*;
use oxc_ast::AstKind;
use oxc_semantic::{AstNodes, NodeId, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::operator::{AssignmentOperator, BinaryOperator, LogicalOperator, UnaryOperator};

use crate::signals::extract::bindings::{numeric_index, object_members_of, ParameterSlots};
use crate::signals::schema::Confidence;
use super::{FinderClaim, FinderCtx, FinderRole, FinderRoleKind};
use crate::analysis;

const URL_PARSER: &str = "url_parser";
const URL_PARSER_ALIAS: &str = "url_parser_alias";
const URL_PATHNAME_DECODER: &str = "url_pathname_escape_decoder";
const URL_RELATIVE_PATHNAME_DECODER: &str = "url_relative_pathname_escape_decoder";
const URL_SEARCH_DECODER: &str = "url_search_escape_decoder";
const URL_HASH_DECODER: &str = "url_hash_escape_decoder";
const URL_PARSER_DOCUMENT: &str = "url_parser_document";
const DEFAULT_PORTS: &str = "default_port_by_protocol";
const URL_PARSE_CACHE: &str = "url_parse_cache";
const ANCHOR_ACCESSORS: &str = "url_parser_anchor_accessors";
const ANCHOR_GETTER: &str = "anchor_href_getter_fallback";
const ANCHOR_SETTER: &str = "anchor_href_setter_fallback";
const ROUTE_MATCHER: &str = "request_route_matcher";
const ROUTE_MATCHER_ALIAS: &str = "request_route_matcher_alias";
const STATIC_RULE: &str = "static_route_rule_test";
const RULE_METHOD: &str = "static_route_rule_method_test";
const RULE_PORT_DEFAULT: &str = "dynamic_route_rule_port_default";
const DYNAMIC_RULE: &str = "dynamic_route_rule_test";
const RULE_METHOD_LOWERCASE: &str = "dynamic_route_rule_method_lowercase";
const PUNYCODE_PREDICATE: &str = "punycode_basic_code_point_predicate";
const PUNYCODE_HOST_PATTERN: &str = "punycode_hostname_pattern";
const PUNYCODE_LABEL_PATTERN: &str = "punycode_label_prefix_pattern";
const DIGIT_START: &str = "punycode_digit_code_point_start";
const DIGIT_END: &str = "punycode_digit_code_point_end";
const LETTER_START: &str = "punycode_letter_code_point_start";
const LETTER_END: &str = "punycode_letter_code_point_end";
const CODE_UNITS_TO_STRING: &str = "punycode_code_units_to_string";
const HOOK_PREDICATE: &str = "request_hook_predicate";
const ROUTE_TABLE: &str = "route_match_table";
const ROUTE_PROVIDER: &str = "dynamic_route_provider";
const ENVELOPE_BUILDER: &str = "request_envelope_builder";
const ENVELOPE_FORWARDER: &str = "request_envelope_forwarder";
const DEFAULT_FILTER: &str = "default_signal_filter";
const SIGNAL_ENCODER: &str = "envelope_signal_encoder";
const ENVELOPE_TOKEN: &str = "envelope_transaction_token";
const CHUNKER: &str = "header_value_chunker";
const LENGTH_LIST: &str = "prefixed_key_length_list";
const LENGTH_ENTRY: &str = "prefixed_key_length_entry";

const fn function(role: &'static str, name: &'static str) -> FinderRole {
    FinderRole { role, name, kind: FinderRoleKind::Function }
}

const fn alias(role: &'static str, name: &'static str) -> FinderRole {
    FinderRole { role, name, kind: FinderRoleKind::FunctionAlias }
}

const fn main(role: &'static str, name: &'static str) -> FinderRole {
    FinderRole { role, name, kind: FinderRoleKind::MainBinding }
}

/// Every role this package can claim, with the one name it receives.
///
/// The MAIN aliases of the hook predicate, the envelope builder, the chunker
/// and the length list are claimed by `request_output_carriers`, which owns those targets.
/// A claim here would be a role conflict that leaves them unnamed.
pub(crate) const VOCABULARY: &[FinderRole] = &[
    function(URL_PARSER, "parseUrl"),
    alias(URL_PARSER_ALIAS, "urlParser"),
    function(URL_PATHNAME_DECODER, "decodePathnameEscape"),
    function(URL_RELATIVE_PATHNAME_DECODER, "decodeRelativePathnameEscape"),
    function(URL_SEARCH_DECODER, "decodeSearchEscape"),
    function(URL_HASH_DECODER, "decodeHashEscape"),
    main(URL_PARSER_DOCUMENT, "urlParserDocument"),
    main(DEFAULT_PORTS, "defaultPortByProtocol"),
    main(URL_PARSE_CACHE, "urlParseCache"),
    main(ANCHOR_ACCESSORS, "anchorAccessor"),
    function(ANCHOR_GETTER, "getAnchorHrefFallback"),
    function(ANCHOR_SETTER, "setAnchorHrefFallback"),
    function(ROUTE_MATCHER, "matchRequestRoutes"),
    alias(ROUTE_MATCHER_ALIAS, "requestRouteMatcher"),
    function(STATIC_RULE, "matchesStaticRouteRule"),
    function(RULE_METHOD, "matchesRuleMethod"),
    function(RULE_PORT_DEFAULT, "withDefaultRulePort"),
    function(DYNAMIC_RULE, "matchesDynamicRouteRule"),
    function(RULE_METHOD_LOWERCASE, "lowercaseRuleMethod"),
    function(PUNYCODE_PREDICATE, "isPunycodeBasicCodePoint"),
    main(PUNYCODE_HOST_PATTERN, "punycodeHostnamePattern"),
    main(PUNYCODE_LABEL_PATTERN, "punycodePrefixPattern"),
    main(DIGIT_START, "digitCodePointStart"),
    main(DIGIT_END, "digitCodePointEnd"),
    main(LETTER_START, "lowercaseLetterCodePointStart"),
    main(LETTER_END, "lowercaseLetterCodePointEnd"),
    main(CODE_UNITS_TO_STRING, "punycodeCodeUnitsToString"),
    function(HOOK_PREDICATE, "shouldHookRequest"),
    main(ROUTE_TABLE, "routeMatchTable"),
    main(ROUTE_PROVIDER, "dynamicRouteProvider"),
    function(ENVELOPE_BUILDER, "getEncodedData"),
    function(ENVELOPE_FORWARDER, "forwardGetEncodedData"),
    main(DEFAULT_FILTER, "defaultSignalFilter"),
    main(SIGNAL_ENCODER, "signalEncoder"),
    main(ENVELOPE_TOKEN, "transactionToken"),
    function(CHUNKER, "chunkHeaderValue"),
    function(LENGTH_LIST, "formatPrefixedKeyLengths"),
    function(LENGTH_ENTRY, "formatPrefixedKeyLength"),
];

/// Structural claims of this package.
pub(crate) fn find(ctx: &FinderCtx) -> Vec<FinderClaim> {
    let index = Index::new(ctx);
    let mut out = Claims::default();
    let count = index.fns.len();
    let parsers: Vec<usize> =
        (0..count).filter(|f| index.claim_url_parser(*f, &mut out)).collect();
    index.claim_anchor_fallbacks(&mut out);
    let matchers: Vec<usize> =
        (0..count).filter(|f| index.claim_route_matcher(*f, &parsers, &mut out)).collect();
    let envelope_builder_candidates: Vec<usize> = (0..count)
        .filter(|f| index.claim_envelope_builder(*f, &mut out))
        .collect();
    let envelope_builders: BTreeSet<usize> = if envelope_builder_candidates.len() == 1 {
        envelope_builder_candidates.into_iter().collect()
    } else {
        BTreeSet::new()
    };
    for f in 0..count {
        index.claim_envelope_forwarder(f, &envelope_builders, &mut out);
        index.claim_hook_predicate(f, &matchers, &mut out);
        index.claim_chunker(f, &mut out);
        index.claim_length_list(f, &mut out);
    }
    out.into_unique()
}

#[derive(Default)]
struct Claims {
    claims: Vec<FinderClaim>,
    seen: BTreeSet<(&'static str, u32, u32)>,
}

impl Claims {
    /// The claims of every role that matched exactly one span. Two candidates
    /// for one role cannot be told apart, so neither is named.
    fn into_unique(self) -> Vec<FinderClaim> {
        let mut spans: BTreeMap<&'static str, usize> = BTreeMap::new();
        for (role, _, _) in &self.seen {
            *spans.entry(*role).or_default() += 1;
        }
        let mut claims = self.claims;
        claims.retain(|claim| spans.get(claim.role()) == Some(&1));
        claims
    }

    fn function(
        &mut self,
        role: &'static str,
        span: Span,
        alias_role: Option<&'static str>,
        evidence: &[&str],
    ) {
        if self.seen.insert((role, span.start, span.end)) {
            self.claims.push(FinderClaim::Function {
                role,
                function_span: span,
                alias_role,
                confidence: Confidence::High,
                evidence: evidence.iter().map(|line| line.to_string()).collect(),
            });
        }
    }

    fn main(&mut self, role: &'static str, span: Span, evidence: &[&str]) {
        if self.seen.insert((role, span.start, span.end)) {
            self.claims.push(FinderClaim::MainBinding {
                role,
                identifier_span: span,
                confidence: Confidence::High,
                evidence: evidence.iter().map(|line| line.to_string()).collect(),
            });
        }
    }
}

// ===========================================================================
// Structural index
// ===========================================================================

/// What one function owns directly, excluding its nested functions.
#[derive(Default)]
struct Own<'a> {
    calls: Vec<&'a CallExpression<'a>>,
    assignments: Vec<&'a AssignmentExpression<'a>>,
    conditionals: Vec<&'a ConditionalExpression<'a>>,
    logicals: Vec<&'a LogicalExpression<'a>>,
    binaries: Vec<&'a BinaryExpression<'a>>,
    objects: Vec<&'a ObjectExpression<'a>>,
    returns: Vec<&'a Expression<'a>>,
    /// `object.name` / `object["name"]` reads and writes.
    members: Vec<(&'a Expression<'a>, &'a str)>,
    /// `return;` statements with no argument.
    bare_returns: usize,
}

struct Unit<'a> {
    node: NodeId,
    span: Span,
    parent: Option<usize>,
    statements: &'a [Statement<'a>],
    own: Own<'a>,
}

struct Index<'a> {
    scoping: &'a Scoping,
    nodes: &'a AstNodes<'a>,
    slots: ParameterSlots<'a, 'a>,
    fns: Vec<Unit<'a>>,
    /// Owning function of every node, by node index.
    owner: Vec<Option<usize>>,
    fn_by_span: BTreeMap<(u32, u32), usize>,
    decl_fn: BTreeMap<SymbolId, usize>,
    /// Initializer and plain `=` values of a binding.
    values: BTreeMap<SymbolId, Vec<&'a Expression<'a>>>,
    /// How many plain `=` assignments a binding has.
    assigned: BTreeMap<SymbolId, usize>,
    /// Every `object.name = value` in the program, as `(name, value)`.
    member_writes: Vec<(&'a str, &'a Expression<'a>)>,
}

fn strip<'e, 'a>(expression: &'e Expression<'a>) -> &'e Expression<'a> {
    match expression {
        Expression::ParenthesizedExpression(inner) => strip(&inner.expression),
        _ => expression,
    }
}

/// `(object, name)` of a static or string-keyed member expression.
fn member_of<'e, 'a>(expression: &'e Expression<'a>) -> Option<(&'e Expression<'a>, &'e str)> {
    match strip(expression) {
        Expression::StaticMemberExpression(member) => {
            Some((&member.object, member.property.name.as_str()))
        }
        Expression::ComputedMemberExpression(member) => match &member.expression {
            Expression::StringLiteral(literal) => Some((&member.object, literal.value.as_str())),
            _ => None,
        },
        _ => None,
    }
}

/// `(object, name)` of a member assignment target.
fn target_member<'e, 'a>(
    target: &'e AssignmentTarget<'a>,
) -> Option<(&'e Expression<'a>, &'e str)> {
    match target {
        AssignmentTarget::StaticMemberExpression(member) => {
            Some((&member.object, member.property.name.as_str()))
        }
        AssignmentTarget::ComputedMemberExpression(member) => match &member.expression {
            Expression::StringLiteral(literal) => Some((&member.object, literal.value.as_str())),
            _ => None,
        },
        _ => None,
    }
}

fn call_arguments<'e, 'a>(call: &'e CallExpression<'a>) -> Vec<&'e Expression<'a>> {
    call.arguments.iter().filter_map(Argument::as_expression).collect()
}

/// `(receiver, method, arguments)` of `receiver.method(...)` and of the traced
/// `receiver["method"].call(receiver, ...)`.
fn method_call<'e, 'a>(
    call: &'e CallExpression<'a>,
) -> Option<(&'e Expression<'a>, &'e str, Vec<&'e Expression<'a>>)> {
    let arguments = call_arguments(call);
    let (object, name) = member_of(&call.callee)?;
    if name == "call" {
        if let Some((receiver, method)) = member_of(object) {
            let rest = arguments.get(1..).unwrap_or(&[]).to_vec();
            return Some((receiver, method, rest));
        }
    }
    Some((object, name, arguments))
}

fn string_value<'e>(expression: &'e Expression<'_>) -> Option<&'e str> {
    match strip(expression) {
        Expression::StringLiteral(literal) => Some(literal.value.as_str()),
        _ => None,
    }
}

fn is_void(expression: &Expression<'_>) -> bool {
    matches!(strip(expression), Expression::UnaryExpression(unary)
        if unary.operator == UnaryOperator::Void)
}

/// The operands of a `+` chain, in source order.
fn additions<'e, 'a>(expression: &'e Expression<'a>, out: &mut Vec<&'e Expression<'a>>) {
    match strip(expression) {
        Expression::BinaryExpression(binary) if binary.operator == BinaryOperator::Addition => {
            additions(&binary.left, out);
            additions(&binary.right, out);
        }
        other => out.push(other),
    }
}

/// The operands of a `||` chain.
fn disjuncts<'e, 'a>(expression: &'e Expression<'a>, out: &mut Vec<&'e Expression<'a>>) {
    match strip(expression) {
        Expression::LogicalExpression(logical) if logical.operator == LogicalOperator::Or => {
            disjuncts(&logical.left, out);
            disjuncts(&logical.right, out);
        }
        other => out.push(other),
    }
}

/// The condition and span of a `while`, `do...while` or `for` statement.
fn loop_condition<'e, 'a>(statement: &'e Statement<'a>) -> Option<(&'e Expression<'a>, Span)> {
    match statement {
        Statement::WhileStatement(loop_) => Some((&loop_.test, loop_.span)),
        Statement::DoWhileStatement(loop_) => Some((&loop_.test, loop_.span)),
        Statement::ForStatement(loop_) => loop_.test.as_ref().map(|test| (test, loop_.span)),
        _ => None,
    }
}

/// `left < right` of a comparison test.
fn less_than<'e, 'a>(
    expression: &'e Expression<'a>,
) -> Option<(&'e Expression<'a>, &'e Expression<'a>)> {
    let Expression::BinaryExpression(binary) = strip(expression) else {
        return None;
    };
    (binary.operator == BinaryOperator::LessThan).then_some((&binary.left, &binary.right))
}

fn literal_number(expression: &Expression<'_>) -> Option<f64> {
    match strip(expression) {
        Expression::NumericLiteral(literal) => Some(literal.value),
        _ => None,
    }
}

/// The single value returned by one branch arm: `return <value>;`, optionally
/// wrapped in exactly one block.
fn branch_return<'e, 'a>(statement: &'e Statement<'a>) -> Option<&'e Expression<'a>> {
    let statement = match statement {
        Statement::BlockStatement(block) => {
            let [statement] = block.body.as_slice() else {
                return None;
            };
            statement
        }
        statement => statement,
    };
    match statement {
        Statement::ReturnStatement(returned) => returned.argument.as_ref(),
        _ => None,
    }
}

/// Whether an expression is a side-effect-free predicate over identifiers and
/// literals: comparisons and `&&`/`||` only.
fn pure_predicate_expression(expression: &Expression<'_>) -> bool {
    match strip(expression) {
        Expression::Identifier(_)
        | Expression::NumericLiteral(_)
        | Expression::BooleanLiteral(_) => true,
        Expression::BinaryExpression(binary) => {
            matches!(
                binary.operator,
                BinaryOperator::GreaterEqualThan
                    | BinaryOperator::LessEqualThan
                    | BinaryOperator::StrictEquality
            ) && pure_predicate_expression(&binary.left)
                && pure_predicate_expression(&binary.right)
        }
        Expression::LogicalExpression(logical) => {
            matches!(logical.operator, LogicalOperator::And | LogicalOperator::Or)
                && pure_predicate_expression(&logical.left)
                && pure_predicate_expression(&logical.right)
        }
        _ => false,
    }
}

impl<'a> Index<'a> {
    fn new(ctx: &FinderCtx<'_, 'a>) -> Self {
        let mut index = Self {
            scoping: ctx.scoping,
            nodes: ctx.nodes,
            slots: ParameterSlots::new(ctx.program, ctx.scoping, ctx.nodes),
            fns: Vec::new(),
            owner: vec![None; ctx.nodes.len()],
            fn_by_span: BTreeMap::new(),
            decl_fn: BTreeMap::new(),
            values: BTreeMap::new(),
            assigned: BTreeMap::new(),
            member_writes: Vec::new(),
        };
        index.build();
        index
    }

    fn build(&mut self) {
        let nodes = self.nodes;
        let mut fn_of_node: Vec<Option<usize>> = vec![None; nodes.len()];
        for (id, node) in nodes.iter_enumerated() {
            let position = id.index();
            let parent = nodes.parent_id(id);
            let owner = if parent == id || parent.index() >= position {
                None
            } else {
                fn_of_node[parent.index()].or(self.owner[parent.index()])
            };
            self.owner[position] = owner;
            let unit = match node.kind() {
                AstKind::Function(function) => Some((
                    function.span,
                    function.body.as_ref().map_or(&[][..], |body| body.statements.as_slice()),
                    function.id.as_ref().and_then(|id| id.symbol_id.get()),
                )),
                AstKind::ArrowFunctionExpression(arrow) => {
                    Some((arrow.span, arrow.body.statements.as_slice(), None))
                }
                _ => None,
            };
            if let Some((span, statements, symbol)) = unit {
                let unit_index = self.fns.len();
                self.fns.push(Unit { node: id, span, parent: owner, statements, own: Own::default() });
                fn_of_node[position] = Some(unit_index);
                self.fn_by_span.insert((span.start, span.end), unit_index);
                if let Some(symbol) = symbol {
                    self.decl_fn.insert(symbol, unit_index);
                }
                if let AstKind::ArrowFunctionExpression(arrow) = node.kind() {
                    if arrow.expression {
                        if let Some(Statement::ExpressionStatement(statement)) =
                            arrow.body.statements.first()
                        {
                            self.fns[unit_index].own.returns.push(&statement.expression);
                        }
                    }
                }
                continue;
            }
            self.record(node.kind(), owner);
        }
    }

    fn record(&mut self, kind: AstKind<'a>, owner: Option<usize>) {
        match kind {
            AstKind::VariableDeclarator(declarator) => {
                if let (BindingPattern::BindingIdentifier(binding), Some(init)) =
                    (&declarator.id, declarator.init.as_ref())
                {
                    if let Some(symbol) = binding.symbol_id.get() {
                        self.values.entry(symbol).or_default().push(init);
                    }
                }
            }
            AstKind::AssignmentExpression(assignment)
                if assignment.operator == AssignmentOperator::Assign =>
            {
                if let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left {
                    if let Some(symbol) = analysis::identifier_symbol(identifier, self.scoping) {
                        self.values.entry(symbol).or_default().push(&assignment.right);
                        *self.assigned.entry(symbol).or_default() += 1;
                    }
                }
                if let Some((_, name)) = target_member(&assignment.left) {
                    self.member_writes.push((name, &assignment.right));
                }
            }
            _ => {}
        }
        let Some(owner) = owner else {
            return;
        };
        let own = &mut self.fns[owner].own;
        match kind {
            AstKind::CallExpression(call) => own.calls.push(call),
            AstKind::AssignmentExpression(assignment) => own.assignments.push(assignment),
            AstKind::ConditionalExpression(conditional) => own.conditionals.push(conditional),
            AstKind::LogicalExpression(logical) => own.logicals.push(logical),
            AstKind::BinaryExpression(binary) => own.binaries.push(binary),
            AstKind::ObjectExpression(object) => own.objects.push(object),
            AstKind::ReturnStatement(statement) => match &statement.argument {
                Some(argument) => own.returns.push(argument),
                None => own.bare_returns += 1,
            },
            AstKind::StaticMemberExpression(member) => {
                own.members.push((&member.object, member.property.name.as_str()));
            }
            AstKind::ComputedMemberExpression(member) => {
                if let Expression::StringLiteral(literal) = &member.expression {
                    own.members.push((&member.object, literal.value.as_str()));
                }
            }
            _ => {}
        }
    }

    // -----------------------------------------------------------------------
    // Resolution
    // -----------------------------------------------------------------------

    fn own(&self, f: usize) -> &Own<'a> {
        &self.fns[f].own
    }

    fn symbol(&self, expression: &Expression<'_>) -> Option<SymbolId> {
        analysis::expression_symbol(strip(expression), self.scoping)
    }

    fn is_global(&self, expression: &Expression<'_>, name: &str) -> bool {
        matches!(strip(expression), Expression::Identifier(identifier)
            if identifier.name == name
                && analysis::identifier_symbol(identifier, self.scoping).is_none())
    }

    /// The one value of a variable with exactly one declaration and one plain
    /// write (initializer or assignment).
    fn single_value(&self, symbol: SymbolId) -> Option<&'a Expression<'a>> {
        let declaration = self.scoping.symbol_declaration(symbol);
        if !matches!(self.nodes.kind(declaration), AstKind::VariableDeclarator(_))
            || self.scoping.symbol_declarations(symbol).count() != 1
        {
            return None;
        }
        let writes = self.scoping.get_resolved_references(symbol).filter(|r| r.is_write()).count();
        if writes != self.assigned.get(&symbol).copied().unwrap_or(0) {
            return None;
        }
        match self.values.get(&symbol)?.as_slice() {
            [value] => Some(*value),
            _ => None,
        }
    }

    /// Whether `symbol` is declared inside function `f` or a function nested
    /// in it.
    fn is_inside(&self, symbol: SymbolId, f: usize) -> bool {
        let declaration = self.scoping.symbol_declaration(symbol);
        let mut current = self.owner[declaration.index()];
        while let Some(index) = current {
            if index == f {
                return true;
            }
            current = self.fns[index].parent;
        }
        false
    }

    /// The binding outside `f` an expression reads, through single-value
    /// locals of `f`.
    fn outer_binding(&self, expression: &Expression<'a>, f: usize) -> Option<SymbolId> {
        let mut symbol = self.symbol(expression)?;
        for _ in 0..8 {
            if !self.is_inside(symbol, f) {
                return Some(symbol);
            }
            symbol = self.symbol(self.single_value(symbol)?)?;
        }
        None
    }

    /// The value behind an expression, through single-value locals of `f`.
    fn local_value<'e>(&self, expression: &'e Expression<'a>, f: usize) -> &'e Expression<'a>
    where
        'a: 'e,
    {
        let mut current = strip(expression);
        for _ in 0..8 {
            let Some(symbol) = self.symbol(current) else {
                break;
            };
            if !self.is_inside(symbol, f) {
                break;
            }
            let Some(value) = self.single_value(symbol) else {
                break;
            };
            current = strip(value);
        }
        current
    }

    /// The function an expression denotes, through single-value bindings.
    fn function_of(&self, expression: &Expression<'a>) -> Option<usize> {
        let mut current = strip(expression);
        for _ in 0..8 {
            match current {
                Expression::FunctionExpression(function) => {
                    return self.fn_by_span.get(&(function.span.start, function.span.end)).copied();
                }
                Expression::ArrowFunctionExpression(arrow) => {
                    return self.fn_by_span.get(&(arrow.span.start, arrow.span.end)).copied();
                }
                _ => {}
            }
            let symbol = self.symbol(current)?;
            if let Some(function) = self.decl_fn.get(&symbol) {
                return Some(*function);
            }
            current = strip(self.single_value(symbol)?);
        }
        None
    }

    /// The one source value of an outer `let` declared without an initializer
    /// and assigned exactly once. This is intentionally narrower than
    /// [`single_value`]: a forwarding wrapper may follow only this scoped,
    /// immutable capture origin, never a parameter, a mutable alias, or a
    /// binding with several possible values.
    fn unique_let_assignment(&self, symbol: SymbolId) -> Option<&'a Expression<'a>> {
        let declaration = self.scoping.symbol_declaration(symbol);
        let AstKind::VariableDeclarator(declarator) = self.nodes.kind(declaration) else {
            return None;
        };
        if declarator.init.is_some()
            || self.scoping.symbol_declarations(symbol).count() != 1
            || !self.scoping.symbol_redeclarations(symbol).is_empty()
            || !matches!(
                self.nodes.parent_kind(declaration),
                AstKind::VariableDeclaration(variable) if variable.kind == VariableDeclarationKind::Let
            )
            || self.assigned.get(&symbol).copied() != Some(1)
        {
            return None;
        }
        let writes = self
            .scoping
            .get_resolved_references(symbol)
            .filter(|reference| reference.is_write())
            .count();
        if writes != 1 {
            return None;
        }
        match self.values.get(&symbol)?.as_slice() {
            [value] => Some(*value),
            _ => None,
        }
    }

    /// The parameter slot of `f` an expression reads, through single-value
    /// locals.
    fn slot(&self, expression: &Expression<'a>, f: usize) -> Option<u32> {
        let mut current = strip(expression);
        for _ in 0..8 {
            if let Some(slot) = self.slots.incoming_slot(current) {
                return (slot.owner == self.fns[f].node).then_some(slot.index);
            }
            current = strip(self.single_value(self.symbol(current)?)?);
        }
        None
    }

    fn is_slot(&self, expression: &Expression<'a>, f: usize, index: u32) -> bool {
        self.slot(expression, f) == Some(index)
    }

    fn is_slot_member(&self, expression: &Expression<'a>, f: usize, index: u32, name: &str) -> bool {
        member_of(expression)
            .is_some_and(|(object, member)| member == name && self.is_slot(object, f, index))
    }

    /// Whether a value is `Object.create(null)`.
    fn is_null_prototype_object(&self, expression: &Expression<'_>) -> bool {
        let Expression::CallExpression(call) = strip(expression) else {
            return false;
        };
        member_of(&call.callee).is_some_and(|(object, name)| {
            name == "create"
                && self.is_global(object, "Object")
                && matches!(
                    call.arguments.first().and_then(Argument::as_expression),
                    Some(Expression::NullLiteral(_))
                )
        })
    }

    /// Whether a value is a regular expression built by `RegExp` or a literal.
    fn is_regexp(&self, expression: &Expression<'_>) -> bool {
        match strip(expression) {
            Expression::RegExpLiteral(_) => true,
            Expression::CallExpression(call) => self.is_global(&call.callee, "RegExp"),
            Expression::NewExpression(call) => self.is_global(&call.callee, "RegExp"),
            _ => false,
        }
    }

    /// Whether a binding holds a regular expression, directly or through a
    /// chain of single-value holders.
    fn holds_regexp(&self, symbol: SymbolId) -> bool {
        let mut current = symbol;
        for _ in 0..8 {
            let Some(value) = self.single_value(current) else {
                return false;
            };
            if self.is_regexp(value) {
                return true;
            }
            let Some(next) = self.symbol(value) else {
                return false;
            };
            current = next;
        }
        false
    }

    /// `f` and every function nested in it.
    fn subtree(&self, f: usize) -> Vec<usize> {
        (0..self.fns.len())
            .filter(|candidate| {
                let mut current = Some(*candidate);
                while let Some(index) = current {
                    if index == f {
                        return true;
                    }
                    current = self.fns[index].parent;
                }
                false
            })
            .collect()
    }

    /// The functions handed to `method` as its first argument in `f`.
    fn callbacks(&self, f: usize, method: &str) -> Vec<usize> {
        let mut found = Vec::new();
        for call in &self.own(f).calls {
            if let Some((_, name, arguments)) = method_call(call) {
                if name == method {
                    found.extend(arguments.first().and_then(|first| self.function_of(first)));
                }
            }
        }
        found
    }

    /// The single returned expression of `f`, when it has exactly one.
    fn single_return(&self, f: usize) -> Option<&'a Expression<'a>> {
        match self.own(f).returns.as_slice() {
            [value] if self.own(f).bare_returns == 0 => Some(*value),
            _ => None,
        }
    }

    /// The local `f` returns whose single value is `Object.create(null)`.
    fn returned_null_object(&self, f: usize) -> Option<SymbolId> {
        self.own(f).returns.iter().find_map(|value| {
            let symbol = self.symbol(value)?;
            self.is_null_prototype_object(self.single_value(symbol)?).then_some(symbol)
        })
    }

    /// The declaring identifier span of a binding.
    fn identifier(&self, symbol: SymbolId) -> Span {
        self.scoping.symbol_span(symbol)
    }

    // -----------------------------------------------------------------------
    // URL parser
    // -----------------------------------------------------------------------

    /// The field names of a local object: its literal keys plus the member
    /// writes `f` makes on it.
    fn object_fields(
        &self,
        local: SymbolId,
        literal: &'a ObjectExpression<'a>,
        f: usize,
    ) -> BTreeSet<String> {
        let mut fields: BTreeSet<String> =
            object_members_of(literal).into_iter().map(|member| member.name).collect();
        for assignment in &self.own(f).assignments {
            if let Some((object, name)) = target_member(&assignment.left) {
                if self.symbol(object) == Some(local) {
                    fields.insert(name.to_string());
                }
            }
        }
        fields
    }

    fn claim_url_parser(&self, f: usize, out: &mut Claims) -> bool {
        let own = self.own(f);
        let Some((anchor, document)) = own.calls.iter().find_map(|call| {
            let (receiver, name, arguments) = method_call(call)?;
            let tag = arguments.first().and_then(|first| string_value(first));
            (name == "createElement" && tag == Some("a")).then_some((call.span, receiver))
        }) else {
            return false;
        };
        let Some((result, literal)) = own.returns.iter().find_map(|value| {
            let symbol = self.symbol(value)?;
            match strip(self.single_value(symbol)?) {
                Expression::ObjectExpression(object) => Some((symbol, &**object)),
                _ => None,
            }
        }) else {
            return false;
        };
        let fields = self.object_fields(result, literal, f);
        let complete = ["protocol", "hostname", "port", "pathname", "search", "hash", "href"]
            .iter()
            .all(|field| fields.contains(*field));
        let reads_anchor = object_members_of(literal).iter().any(|member| {
            member.name == "hostname"
                && member_of(member.value).is_some_and(|(object, name)| {
                    name == "hostname"
                        && matches!(self.local_value(object, f), Expression::CallExpression(call)
                            if call.span == anchor)
                })
        });
        if !complete || !reads_anchor {
            return false;
        }
        out.function(
            URL_PARSER,
            self.fns[f].span,
            Some(URL_PARSER_ALIAS),
            &[
                "creates an anchor with createElement(\"a\") and reads its URL fields",
                "returns an object with protocol, hostname, port, pathname, search, hash and href",
            ],
        );
        self.claim_url_state(f, document, result, out);
        self.claim_escape_decoders(f, out);
        true
    }

    fn claim_url_state(
        &self,
        f: usize,
        document: &Expression<'a>,
        result: SymbolId,
        out: &mut Claims,
    ) {
        let own = self.own(f);
        if let Some(symbol) = self.outer_binding(document, f) {
            if self.single_value(symbol).is_some_and(|value| self.is_global(value, "document")) {
                out.main(
                    URL_PARSER_DOCUMENT,
                    self.identifier(symbol),
                    &["holds the global document; the URL parser creates its anchor from it"],
                );
            }
        }
        for binary in &own.binaries {
            if binary.operator != BinaryOperator::StrictEquality {
                continue;
            }
            let Expression::ComputedMemberExpression(lookup) = strip(&binary.right) else {
                continue;
            };
            let compares_port = member_of(&binary.left).is_some_and(|(_, name)| name == "port");
            if !compares_port {
                continue;
            }
            let Some(symbol) = self.outer_binding(&lookup.object, f) else {
                continue;
            };
            let Some(Expression::ObjectExpression(table)) = self.single_value(symbol).map(strip)
            else {
                continue;
            };
            let names: BTreeSet<String> =
                object_members_of(table).into_iter().map(|member| member.name).collect();
            if names.contains("http") && names.contains("https") {
                out.main(
                    DEFAULT_PORTS,
                    self.identifier(symbol),
                    &["protocol-keyed object; the URL parser compares the anchor port with its entry to drop a default port"],
                );
            }
        }
        for assignment in &own.assignments {
            let AssignmentTarget::ComputedMemberExpression(target) = &assignment.left else {
                continue;
            };
            if self.symbol(&assignment.right) != Some(result) {
                continue;
            }
            let Some(symbol) = self.outer_binding(&target.object, f) else {
                continue;
            };
            if self.single_value(symbol).is_some_and(|value| self.is_null_prototype_object(value)) {
                out.main(
                    URL_PARSE_CACHE,
                    self.identifier(symbol),
                    &["null-prototype object; the URL parser stores its result there by URL"],
                );
            }
        }
        for logical in &own.logicals {
            if logical.operator != LogicalOperator::Coalesce || !self.is_slot(&logical.left, f, 1) {
                continue;
            }
            let used = own.calls.iter().any(|call| {
                method_call(call).is_some_and(|(receiver, name, _)| {
                    name == "anchorHrefSet" && self.local_value(receiver, f).span() == logical.span
                })
            });
            if let (true, Some(symbol)) = (used, self.outer_binding(&logical.right, f)) {
                out.main(
                    ANCHOR_ACCESSORS,
                    self.identifier(symbol),
                    &["default of the URL parser's accessor slot; it calls anchorHrefSet on the value"],
                );
            }
        }
    }

    fn claim_escape_decoders(&self, f: usize, out: &mut Claims) {
        let own = self.own(f);
        let relative: BTreeSet<(u32, u32)> = own
            .binaries
            .iter()
            .filter(|binary| {
                binary.operator == BinaryOperator::Addition
                    && string_value(&binary.left) == Some("/")
            })
            .map(|binary| {
                let span = strip(&binary.right).span();
                (span.start, span.end)
            })
            .collect();
        for call in &own.calls {
            let Some((receiver, "replace", arguments)) = method_call(call) else {
                continue;
            };
            let (Some((_, field)), Some(callback)) = (member_of(receiver), arguments.get(1)) else {
                continue;
            };
            let Some(decoder) = self.function_of(callback) else {
                continue;
            };
            if !self.is_escape_decoder(decoder) {
                continue;
            }
            let role = match field {
                "pathname" if relative.contains(&(call.span.start, call.span.end)) => {
                    URL_RELATIVE_PATHNAME_DECODER
                }
                "pathname" => URL_PATHNAME_DECODER,
                "search" => URL_SEARCH_DECODER,
                "hash" => URL_HASH_DECODER,
                _ => continue,
            };
            out.function(
                role,
                self.fns[decoder].span,
                None,
                &["replace callback on an anchor URL field of the URL parser; returns decodeURIComponent of its match"],
            );
        }
    }

    fn is_escape_decoder(&self, f: usize) -> bool {
        let Some(Expression::CallExpression(call)) = self.single_return(f).map(strip) else {
            return false;
        };
        let arguments = call_arguments(call);
        self.is_global(&call.callee, "decodeURIComponent")
            && arguments.len() == 1
            && self.is_slot(arguments[0], f, 0)
    }

    fn claim_anchor_fallbacks(&self, out: &mut Claims) {
        for (name, value) in &self.member_writes {
            let (role, setter) = match *name {
                "anchorHrefGet" => (ANCHOR_GETTER, false),
                "anchorHrefSet" => (ANCHOR_SETTER, true),
                _ => continue,
            };
            // The installed value may be a binding with several writes: the
            // native accessor in one branch and the fallback in the other.
            let candidates: Vec<usize> = match self.symbol(value) {
                Some(symbol) if !self.decl_fn.contains_key(&symbol) => self
                    .values
                    .get(&symbol)
                    .map(|values| values.iter().filter_map(|v| self.function_of(v)).collect())
                    .unwrap_or_default(),
                _ => self.function_of(value).into_iter().collect(),
            };
            for f in candidates {
                let proven = if setter { self.is_href_setter(f) } else { self.is_href_getter(f) };
                if proven {
                    out.function(
                        role,
                        self.fns[f].span,
                        None,
                        &["installed as the anchor href accessor; plain href member access on its element slot"],
                    );
                }
            }
        }
    }

    fn is_href_getter(&self, f: usize) -> bool {
        self.fns[f].statements.len() == 1
            && self.own(f).calls.is_empty()
            && self.single_return(f).is_some_and(|value| self.is_slot_member(value, f, 0, "href"))
    }

    fn is_href_setter(&self, f: usize) -> bool {
        let own = self.own(f);
        own.calls.is_empty()
            && matches!(own.assignments.as_slice(), [assignment]
                if self.is_slot(&assignment.right, f, 1)
                    && target_member(&assignment.left).is_some_and(|(object, name)| {
                        name == "href" && self.is_slot(object, f, 0)
                    }))
            && own.returns.iter().all(|value| is_void(value))
    }

    // -----------------------------------------------------------------------
    // Route matcher and punycode branch
    // -----------------------------------------------------------------------

    fn claim_route_matcher(&self, f: usize, parsers: &[usize], out: &mut Claims) -> bool {
        let own = self.own(f);
        let parses_url = own.calls.iter().any(|call| {
            let arguments = call_arguments(call);
            self.function_of(&call.callee).is_some_and(|callee| parsers.contains(&callee))
                && arguments.first().is_some_and(|first| self.is_slot_member(first, f, 0, "url"))
        });
        let reads_method = own.calls.iter().any(|call| {
            method_call(call).is_some_and(|(receiver, name, _)| {
                name == "toLowerCase" && self.is_slot_member(receiver, f, 0, "method")
            })
        });
        if !parses_url || !reads_method {
            return false;
        }
        let tree = self.subtree(f);
        let rules: Vec<usize> = tree.iter().flat_map(|unit| self.callbacks(*unit, "some")).collect();
        let static_rules: Vec<usize> =
            rules.iter().copied().filter(|rule| self.is_static_rule(*rule)).collect();
        if static_rules.is_empty() {
            return false;
        }
        out.function(
            ROUTE_MATCHER,
            self.fns[f].span,
            Some(ROUTE_MATCHER_ALIAS),
            &[
                "parses its request slot's url with the proven URL parser and lowercases its method",
                "a some() rule callback tests protocol, hostname, port, pathname and search",
            ],
        );
        for rule in &static_rules {
            out.function(
                STATIC_RULE,
                self.fns[*rule].span,
                None,
                &["some() callback of the route matcher; tests all five parsed URL fields against table regexes"],
            );
            for method in self.callbacks(*rule, "some") {
                if self.is_method_test(method) {
                    out.function(
                        RULE_METHOD,
                        self.fns[method].span,
                        None,
                        &["some() callback of a static rule; compares a lowercased method token with \"any\" and the request method"],
                    );
                }
            }
        }
        for rule in rules.iter().copied().filter(|rule| self.is_dynamic_rule(*rule)) {
            out.function(
                DYNAMIC_RULE,
                self.fns[rule].span,
                None,
                &["some() callback of the route matcher; tests the rule's protocol, host, path and port regexes"],
            );
            for lowercase in self.callbacks(rule, "map") {
                if self.is_lowercase(lowercase) {
                    out.function(
                        RULE_METHOD_LOWERCASE,
                        self.fns[lowercase].span,
                        None,
                        &["map() callback of a dynamic rule; returns its slot lowercased"],
                    );
                }
            }
        }
        for unit in &tree {
            for mapper in self.callbacks(*unit, "map") {
                if self.is_port_default(mapper) {
                    out.function(
                        RULE_PORT_DEFAULT,
                        self.fns[mapper].span,
                        None,
                        &["map() callback of the route matcher; copies a rule with a match-all port regex when port is missing"],
                    );
                }
            }
        }
        self.claim_punycode(f, &tree, out);
        true
    }

    /// Whether `f` tests all five parsed URL fields with `regex.test(field)`.
    fn is_static_rule(&self, f: usize) -> bool {
        let mut tested = BTreeSet::new();
        for call in &self.own(f).calls {
            if let Some((_, "test", arguments)) = method_call(call) {
                if let Some((_, field)) = arguments.first().and_then(|first| member_of(first)) {
                    tested.insert(field);
                }
            }
        }
        ["protocol", "hostname", "port", "pathname", "search"]
            .iter()
            .all(|field| tested.contains(field))
    }

    /// Whether `f` calls `slot.field.test(...)` for each dynamic rule field.
    fn is_dynamic_rule(&self, f: usize) -> bool {
        let mut tested = BTreeSet::new();
        for call in &self.own(f).calls {
            if let Some((receiver, "test", _)) = method_call(call) {
                if let Some((object, field)) = member_of(receiver) {
                    if self.is_slot(object, f, 0) {
                        tested.insert(field);
                    }
                }
            }
        }
        ["protocol", "host", "path", "port"].iter().all(|field| tested.contains(field))
    }

    /// `slot.toLowerCase()`.
    fn is_slot_lowercase(&self, expression: &Expression<'a>, f: usize) -> bool {
        let Expression::CallExpression(call) = strip(expression) else {
            return false;
        };
        member_of(&call.callee)
            .is_some_and(|(object, name)| name == "toLowerCase" && self.is_slot(object, f, 0))
    }

    /// Compares `slot.toLowerCase()` with `"any"` and with one other value,
    /// joined by `||` or written as an `if` with two returns.
    fn is_method_test(&self, f: usize) -> bool {
        let own = self.own(f);
        let comparisons = || {
            own.binaries.iter().filter(|binary| {
                binary.operator == BinaryOperator::StrictEquality
                    && self.is_slot_lowercase(&binary.left, f)
            })
        };
        own.binaries.len() == 2
            && comparisons().count() == 2
            && comparisons().any(|binary| string_value(&binary.right) == Some("any"))
    }

    fn is_lowercase(&self, f: usize) -> bool {
        self.single_return(f).is_some_and(|value| self.is_slot_lowercase(value, f))
    }

    fn is_port_default(&self, f: usize) -> bool {
        let defaults = self.own(f).conditionals.iter().any(|conditional| {
            let Expression::BinaryExpression(test) = strip(&conditional.test) else {
                return false;
            };
            self.is_slot_member(&test.left, f, 0, "port")
                && matches!(strip(&test.right), Expression::NullLiteral(_))
                && self.is_regexp(&conditional.consequent)
                && self.is_slot_member(&conditional.alternate, f, 0, "port")
        });
        let merges = matches!(self.single_return(f).map(strip), Some(Expression::CallExpression(call))
            if call_arguments(call).iter().any(|argument| self.is_slot(argument, f, 0)));
        defaults && merges
    }

    fn claim_punycode(&self, matcher: usize, tree: &[usize], out: &mut Claims) {
        // The code-point predicate is what proves this is a punycode branch;
        // a regex tested on the hostname says nothing about punycode alone.
        let mut proven = false;
        for unit in tree {
            for predicate in self.callbacks(*unit, "every") {
                proven |= self.claim_code_point_predicate(predicate, out);
            }
        }
        if !proven {
            return;
        }
        for unit in tree {
            for call in &self.own(*unit).calls {
                if let Some((receiver, "test", arguments)) = method_call(call) {
                    self.claim_punycode_pattern(matcher, receiver, arguments.first().copied(), out);
                }
                self.claim_code_units_to_string(matcher, call, out);
            }
        }
    }

    fn claim_punycode_pattern(
        &self,
        matcher: usize,
        receiver: &Expression<'a>,
        argument: Option<&Expression<'a>>,
        out: &mut Claims,
    ) {
        let (Some(argument), Some(symbol)) = (argument, self.outer_binding(receiver, matcher)) else {
            return;
        };
        if !self.holds_regexp(symbol) {
            return;
        }
        if member_of(argument).is_some_and(|(_, field)| field == "hostname") {
            out.main(
                PUNYCODE_HOST_PATTERN,
                self.identifier(symbol),
                &["regex the route matcher tests against the parsed hostname to enter its punycode branch"],
            );
        } else if matches!(strip(argument), Expression::ComputedMemberExpression(member)
            if numeric_index(&member.expression).is_none())
        {
            out.main(
                PUNYCODE_LABEL_PATTERN,
                self.identifier(symbol),
                &["regex the punycode branch tests against each hostname label"],
            );
        }
    }

    /// `K(codes)` in the punycode branch, where the alias `K` holds a function
    /// that maps char codes through `String.fromCharCode` and joins them.
    fn claim_code_units_to_string(
        &self,
        matcher: usize,
        call: &CallExpression<'a>,
        out: &mut Claims,
    ) {
        let Some(symbol) = self.outer_binding(&call.callee, matcher) else {
            return;
        };
        if self.decl_fn.contains_key(&symbol) {
            return;
        }
        let Some(target) = self.single_value(symbol).and_then(|value| self.function_of(value)) else {
            return;
        };
        if self.is_code_units_to_string(target) {
            out.main(
                CODE_UNITS_TO_STRING,
                self.identifier(symbol),
                &["alias the punycode branch calls; the held function joins String.fromCharCode of each element"],
            );
        }
    }

    fn is_code_units_to_string(&self, f: usize) -> bool {
        let converts = self.subtree(f).iter().any(|unit| {
            self.own(*unit).calls.iter().any(|call| {
                member_of(&call.callee).is_some_and(|(object, name)| {
                    name == "fromCharCode" && self.is_global(object, "String")
                })
            })
        });
        let joins = self.own(f).calls.iter().any(|call| {
            method_call(call).is_some_and(|(_, name, arguments)| {
                name == "join" && arguments.first().and_then(|first| string_value(first)) == Some("")
            })
        });
        converts && joins
    }

    /// The `(test, alternate)` of the one canonical if/else predicate
    /// `if (ranges) { return true; } else { return <exact>; }`.
    ///
    /// The body must be exactly that one owned `IfStatement`, each arm exactly
    /// one `return <value>;` (optionally inside one block), the consequent
    /// `true`, and both expressions side-effect free. The caller reuses the
    /// existing range/bound/exact proof by reading `test || <exact>`.
    fn if_else_predicate(&self, f: usize) -> Option<(&Expression<'a>, &Expression<'a>)> {
        let [Statement::IfStatement(branch)] = self.fns[f].statements else {
            return None;
        };
        let yes = branch_return(&branch.consequent)?;
        let no = branch_return(branch.alternate.as_ref()?)?;
        if !matches!(strip(yes), Expression::BooleanLiteral(value) if value.value)
            || !pure_predicate_expression(&branch.test)
            || !pure_predicate_expression(no)
        {
            return None;
        }
        Some((&branch.test, no))
    }

    /// `x >= A && x <= B || x >= C && x <= D || x === hyphen`, with each bound
    /// classified by the single character it is the code of.
    ///
    /// The donor spelling is the single `return` above. The if/else spelling
    /// is read through [`Self::if_else_predicate`] and reuses the same proof.
    fn claim_code_point_predicate(&self, f: usize, out: &mut Claims) -> bool {
        let mut parts = Vec::new();
        if let Some(value) = self.single_return(f) {
            disjuncts(value, &mut parts);
        } else if let Some((test, alternate)) = self.if_else_predicate(f) {
            disjuncts(test, &mut parts);
            parts.push(alternate);
        } else {
            return false;
        }
        if !self.same_predicate_input(&parts, f) {
            return false;
        }
        let ranges: Vec<(SymbolId, SymbolId)> =
            parts.iter().filter_map(|part| self.code_point_range(part, f)).collect();
        let exact = parts.iter().any(|part| self.is_exact_code_point(part, f));
        if parts.len() != 3 || ranges.len() != 2 || !exact {
            return false;
        }
        let mut bounds: Vec<(&'static str, SymbolId)> = Vec::new();
        for (low, high) in ranges {
            match (self.bound_character(low), self.bound_character(high)) {
                (Some('0'), Some('9')) => bounds.extend([(DIGIT_START, low), (DIGIT_END, high)]),
                (Some('a'), Some('z')) => bounds.extend([(LETTER_START, low), (LETTER_END, high)]),
                _ => return false,
            }
        }
        if bounds.iter().filter(|(role, _)| *role == DIGIT_START).count() != 1 {
            return false;
        }
        out.function(
            PUNYCODE_PREDICATE,
            self.fns[f].span,
            None,
            &["every() predicate of the punycode branch; accepts the digit and lowercase-letter code ranges and one exact code"],
        );
        for (role, symbol) in bounds {
            out.main(
                role,
                self.identifier(symbol),
                &["range bound of the punycode basic-code-point predicate; holds the char code of that range's end character"],
            );
        }
        true
    }

    /// Whether every comparison in the predicate reads the same input symbol,
    /// following single-value locals to its origin. A foreign parameter or an
    /// unrelated input refuses.
    fn same_predicate_input(&self, parts: &[&Expression<'a>], f: usize) -> bool {
        let mut input = None;
        for part in parts {
            let mut comparisons = Vec::with_capacity(2);
            match strip(part) {
                Expression::LogicalExpression(logical)
                    if logical.operator == LogicalOperator::And =>
                {
                    comparisons.push(&logical.left);
                    comparisons.push(&logical.right);
                }
                other => comparisons.push(other),
            }
            for comparison in comparisons {
                let Expression::BinaryExpression(binary) = strip(comparison) else {
                    return false;
                };
                let Some(symbol) = self.predicate_input(&binary.left, f) else {
                    return false;
                };
                if input.is_some_and(|existing| existing != symbol) {
                    return false;
                }
                input = Some(symbol);
            }
        }
        input.is_some()
    }

    /// The binding a comparison reads as its input, through single-value locals.
    fn predicate_input(&self, expression: &Expression<'a>, f: usize) -> Option<SymbolId> {
        let mut symbol = self.symbol(expression)?;
        for _ in 0..8 {
            if !self.is_inside(symbol, f) {
                break;
            }
            let Some(value) = self.single_value(symbol) else {
                break;
            };
            let Some(next) = self.symbol(value) else {
                break;
            };
            symbol = next;
        }
        Some(symbol)
    }

    /// `slot === 45`, the exact hyphen code shared by both spellings.
    fn is_exact_code_point(&self, expression: &Expression<'a>, f: usize) -> bool {
        let Expression::BinaryExpression(binary) = strip(expression) else {
            return false;
        };
        binary.operator == BinaryOperator::StrictEquality
            && self.is_slot(&binary.left, f, 0)
            && literal_number(&binary.right) == Some(45.0)
    }

    /// `(A, B)` of `slot >= A && slot <= B`.
    fn code_point_range(
        &self,
        expression: &Expression<'a>,
        f: usize,
    ) -> Option<(SymbolId, SymbolId)> {
        let Expression::LogicalExpression(logical) = strip(expression) else {
            return None;
        };
        if logical.operator != LogicalOperator::And {
            return None;
        }
        let bound = |side: &Expression<'a>, operator: BinaryOperator| -> Option<SymbolId> {
            let Expression::BinaryExpression(binary) = strip(side) else {
                return None;
            };
            if binary.operator != operator || !self.is_slot(&binary.left, f, 0) {
                return None;
            }
            self.outer_binding(&binary.right, f)
        };
        Some((
            bound(&logical.left, BinaryOperator::GreaterEqualThan)?,
            bound(&logical.right, BinaryOperator::LessEqualThan)?,
        ))
    }

    /// The character of a binding whose value is `code("c")`.
    fn bound_character(&self, symbol: SymbolId) -> Option<char> {
        let Expression::CallExpression(call) = strip(self.single_value(symbol)?) else {
            return None;
        };
        let arguments = call_arguments(call);
        let [argument] = arguments.as_slice() else {
            return None;
        };
        let mut characters = string_value(argument)?.chars();
        let character = characters.next()?;
        characters.next().is_none().then_some(character)
    }

    // -----------------------------------------------------------------------
    // Hook predicate
    // -----------------------------------------------------------------------

    fn claim_hook_predicate(&self, f: usize, matchers: &[usize], out: &mut Claims) {
        if self.fns[f].statements.len() != 1 {
            return;
        }
        let Some(Expression::CallExpression(call)) = self.single_return(f).map(strip) else {
            return;
        };
        let Some(matcher) = self.function_of(&call.callee).filter(|m| matchers.contains(m)) else {
            return;
        };
        let arguments = call_arguments(call);
        let [request, table, dynamic, provider] = arguments.as_slice() else {
            return;
        };
        if !self.is_slot(request, f, 0) || !matches!(strip(dynamic), Expression::BooleanLiteral(_)) {
            return;
        }
        out.function(
            HOOK_PREDICATE,
            self.fns[f].span,
            None,
            &["returns the proven route matcher applied to its request slot, a table, a mode flag and a provider"],
        );
        if let Some(symbol) = self.outer_binding(table, f) {
            out.main(
                ROUTE_TABLE,
                self.identifier(symbol),
                &["route table slot of the route matcher, passed by the hook predicate"],
            );
        }
        if let Some(symbol) = self.outer_binding(provider, f) {
            if self.calls_slot(matcher, 3) {
                out.main(
                    ROUTE_PROVIDER,
                    self.identifier(symbol),
                    &["provider slot of the route matcher, which calls that slot in its dynamic branch"],
                );
            }
        }
    }

    /// Whether `f` calls parameter `slot` or a local that takes its value.
    fn calls_slot(&self, f: usize, slot: u32) -> bool {
        self.own(f).calls.iter().any(|call| {
            self.is_slot(&call.callee, f, slot)
                || self.symbol(&call.callee).is_some_and(|symbol| {
                    self.values.get(&symbol).is_some_and(|values| {
                        values.iter().any(|value| self.is_slot(value, f, slot))
                    })
                })
        })
    }

    // -----------------------------------------------------------------------
    // Envelope builder
    // -----------------------------------------------------------------------

    /// The local holding `slot != null ? slot : Object.create(null)`.
    fn options_local(&self, f: usize) -> Option<SymbolId> {
        self.own(f).members.iter().find_map(|(object, name)| {
            if *name != "signalFilter" {
                return None;
            }
            let symbol = self.symbol(object)?;
            let Expression::ConditionalExpression(value) = strip(self.single_value(symbol)?) else {
                return None;
            };
            (self.is_slot(&value.consequent, f, 0)
                && self.is_null_prototype_object(&value.alternate))
            .then_some(symbol)
        })
    }

    fn claim_envelope_builder(&self, f: usize, out: &mut Claims) -> bool {
        let own = self.own(f);
        let Some(options) = self.options_local(f) else {
            return false;
        };
        let read: BTreeSet<&str> = own
            .members
            .iter()
            .filter(|(object, _)| self.symbol(object) == Some(options))
            .map(|(_, name)| *name)
            .collect();
        // The named base carries notifyLastChance inside the collector options
        // literal. The base64url variant instead assigns it onto the same
        // collector local after the literal and emits the inline sextet loop.
        let collects_literal = own.objects.iter().copied().any(|object| {
            let names: BTreeSet<String> =
                object_members_of(object).into_iter().map(|member| member.name).collect();
            ["signalFilter", "includeDebugNames", "notifyLastChance"]
                .iter()
                .all(|name| names.contains(*name))
        });
        let notify_after = own.assignments.iter().any(|assignment| {
            target_member(&assignment.left).is_some_and(|(object, name)| {
                name == "notifyLastChance"
                    && self.symbol(object).is_some_and(|collector| {
                        self.is_inside(collector, f) && self.holds_collector_literal(collector)
                    })
            })
        });
        let complete = ["signalFilter", "encode", "notifyLastChance", "descriptor"]
            .iter()
            .all(|name| read.contains(name));
        let Some(result) = self.returned_null_object(f) else {
            return false;
        };
        let writes: Vec<(&'a str, &'a Expression<'a>)> = own
            .assignments
            .iter()
            .filter_map(|assignment| {
                let (object, name) = target_member(&assignment.left)?;
                (self.symbol(object) == Some(result)).then_some((name, &assignment.right))
            })
            .collect();
        let base64url = notify_after && self.base64url_emission(f, &writes);
        if !complete || !(collects_literal || base64url) || writes.len() < 6 {
            return false;
        }
        let mut evidence = vec![
            "reads signalFilter, encode, notifyLastChance and descriptor from its options slot",
            "builds the collector options object and returns a null-prototype object it fills",
        ];
        if base64url {
            evidence.push(
                "emits a %3-grouped charAt sextet loop over the encoder output, appends it into the packet payload `a` with one alphabet symbol across the body, and hashes that payload with the same captured token written to packet field `f` for `b`",
            );
        }
        out.function(ENVELOPE_BUILDER, self.fns[f].span, None, &evidence);
        self.claim_envelope_state(f, &writes, out);
        true
    }

    /// Prove a distinct one-hop semantic forwarder after the envelope builder
    /// itself has been proven. The wrapper is never claimed as the builder:
    /// its callee must resolve by symbol and unique source origin to one of the
    /// already-proven builder function spans.
    fn claim_envelope_forwarder(
        &self,
        f: usize,
        envelope_builders: &BTreeSet<usize>,
        out: &mut Claims,
    ) {
        let parameters = match self.nodes.kind(self.fns[f].node) {
            AstKind::Function(function) => {
                if function.r#async || function.generator {
                    return;
                }
                &function.params
            }
            AstKind::ArrowFunctionExpression(arrow) => {
                if arrow.r#async {
                    return;
                }
                &arrow.params
            }
            _ => return,
        };
        let [parameter] = parameters.items.as_slice() else {
            return;
        };
        if parameters.rest.is_some()
            || parameter.initializer.is_some()
            || !matches!(&parameter.pattern, BindingPattern::BindingIdentifier(_))
        {
            return;
        }

        let own = self.own(f);
        if self.fns[f].statements.len() != 1
            || own.calls.len() != 1
            || !own.assignments.is_empty()
            || !own.conditionals.is_empty()
            || !own.logicals.is_empty()
            || !own.binaries.is_empty()
            || !own.objects.is_empty()
            || !own.members.is_empty()
            || own.bare_returns != 0
        {
            return;
        }
        let Some(Expression::CallExpression(call)) = self.single_return(f).map(strip) else {
            return;
        };
        if call.optional {
            return;
        }
        let [argument] = call.arguments.as_slice() else {
            return;
        };
        let Some(argument) = argument.as_expression() else {
            return;
        };
        if !self.is_slot(argument, f, 0) {
            return;
        }
        let Expression::Identifier(_) = strip(&call.callee) else {
            return;
        };
        let Some(captured) = self.symbol(&call.callee) else {
            return;
        };
        if self.is_inside(captured, f) {
            return;
        }
        let Some(origin) = self.unique_let_assignment(captured) else {
            return;
        };
        let Some(callee) = self.function_of(origin) else {
            return;
        };
        if !envelope_builders.contains(&callee) {
            return;
        }
        out.function(
            ENVELOPE_FORWARDER,
            self.fns[f].span,
            None,
            &[
                "forwards its only plain formal unchanged in one direct call",
                "captured callee is an outer let without initializer with one resolved assignment",
                "captured callee resolves by source identity to the proven request-envelope builder",
            ],
        );
    }

    /// Whether the binding is ever assigned the collector options literal that
    /// carries signalFilter and includeDebugNames. Locals with several writes
    /// are allowed here: the same local first holds the resolved filter value
    /// and is then overwritten with the collector literal.
    fn holds_collector_literal(&self, symbol: SymbolId) -> bool {
        self.values.get(&symbol).is_some_and(|values| {
            values.iter().any(|value| {
                let Expression::ObjectExpression(literal) = strip(value) else {
                    return false;
                };
                let names: BTreeSet<String> =
                    object_members_of(literal).into_iter().map(|member| member.name).collect();
                names.contains("signalFilter") && names.contains("includeDebugNames")
            })
        })
    }

    /// The inline base64url sextet emission of the base64url envelope variant.
    ///
    /// The facts are proved from the owning body only:
    /// - the grouping is exactly `bytes.length % 3`, where `bytes` receives the
    ///   encoder call, and the loop bound is `bytes.length - bytes.length % 3`;
    /// - the loop index steps by three and indexes those same bytes;
    /// - at least three `charAt` sextets are appended into the payload inside
    ///   that loop, at least one of them reads the loop-indexed bytes, and every
    ///   `charAt` in the body uses the same alphabet symbol;
    /// - the packet `a` field is that appended payload and the `b` integrity
    ///   field hashes it with the captured token written to packet field `f`.
    ///
    /// No alphabet, key, constant, span, name or vendor is consulted.
    fn base64url_emission(
        &self,
        f: usize,
        writes: &[(&'a str, &'a Expression<'a>)],
    ) -> bool {
        let own = self.own(f);
        let Some(payload) = writes
            .iter()
            .find(|(name, _)| *name == "a")
            .and_then(|(_, value)| self.symbol(value))
        else {
            return false;
        };
        if !self.is_inside(payload, f) {
            return false;
        }
        let Some(captured) = writes
            .iter()
            .find(|(name, _)| *name == "f")
            .and_then(|(_, value)| self.outer_binding(value, f))
            .filter(|symbol| !self.is_inside(*symbol, f))
        else {
            return false;
        };

        let grouped = self.fns[f].statements.iter().any(|statement| {
            let Some((test, loop_span)) = loop_condition(statement) else {
                return false;
            };
            let Some((index_expression, bound)) = less_than(test) else {
                return false;
            };
            let Some(index) = self.symbol(index_expression) else {
                return false;
            };
            let Expression::BinaryExpression(difference) = strip(self.local_value(bound, f)) else {
                return false;
            };
            if difference.operator != BinaryOperator::Subtraction {
                return false;
            }
            let Expression::BinaryExpression(remainder) = strip(&difference.right) else {
                return false;
            };
            if remainder.operator != BinaryOperator::Remainder
                || literal_number(&remainder.right) != Some(3.0)
            {
                return false;
            }
            let Some((length_object, "length")) = member_of(&remainder.left) else {
                return false;
            };
            let Some(bytes) = self.symbol(length_object) else {
                return false;
            };
            if !self.is_inside(index, f)
                || !self.is_inside(bytes, f)
                || !self.is_length_of(&difference.left, bytes)
                || !self.holds_call_value(bytes)
                || !self.steps_by_three(own, index, loop_span)
            {
                return false;
            }
            let Some(alphabet) = self.payload_alphabet(own, payload, index, bytes, loop_span)
            else {
                return false;
            };
            let mut sextets = 0usize;
            for call in &own.calls {
                let Some((receiver, "charAt", _)) = method_call(call) else {
                    continue;
                };
                if self.symbol(receiver) != Some(alphabet) {
                    return false;
                }
                sextets += 1;
            }
            sextets >= 4
        });
        if !grouped {
            return false;
        }

        writes
            .iter()
            .find(|(name, _)| *name == "b")
            .map(|(_, value)| *value)
            .is_some_and(|value| {
                let Expression::CallExpression(to_string) = strip(value) else {
                    return false;
                };
                let Some((receiver, "toString", arguments)) = method_call(to_string) else {
                    return false;
                };
                if !arguments
                    .first()
                    .is_some_and(|argument| literal_number(argument).is_some())
                {
                    return false;
                }
                let Expression::CallExpression(hash) = strip(receiver) else {
                    return false;
                };
                let Some(Expression::BinaryExpression(sum)) =
                    call_arguments(hash).first().map(|argument| strip(argument))
                else {
                    return false;
                };
                if sum.operator != BinaryOperator::Addition {
                    return false;
                }
                let token_matches = |expression: &Expression<'a>| {
                    self.outer_binding(expression, f) == Some(captured)
                };
                (self.symbol(&sum.left) == Some(payload) && token_matches(&sum.right))
                    || (self.symbol(&sum.right) == Some(payload) && token_matches(&sum.left))
            })
    }

    /// Whether `expression` is `symbol.length`.
    fn is_length_of(&self, expression: &Expression<'_>, symbol: SymbolId) -> bool {
        member_of(expression)
            .is_some_and(|(object, name)| name == "length" && self.symbol(object) == Some(symbol))
    }

    /// Whether `symbol` ever receives a call result, i.e. the encoder output.
    fn holds_call_value(&self, symbol: SymbolId) -> bool {
        self.values.get(&symbol).is_some_and(|values| {
            values
                .iter()
                .any(|value| matches!(strip(value), Expression::CallExpression(_)))
        })
    }

    /// Whether the loop index is advanced by exactly three inside the loop.
    fn steps_by_three(&self, own: &Own<'a>, index: SymbolId, loop_span: Span) -> bool {
        own.assignments.iter().any(|assignment| {
            if !(loop_span.start <= assignment.span.start
                && assignment.span.end <= loop_span.end)
            {
                return false;
            }
            let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else {
                return false;
            };
            if analysis::identifier_symbol(target, self.scoping) != Some(index) {
                return false;
            }
            match assignment.operator {
                AssignmentOperator::Addition => {
                    literal_number(&assignment.right) == Some(3.0)
                }
                AssignmentOperator::Assign => matches!(
                    strip(&assignment.right),
                    Expression::BinaryExpression(sum)
                        if sum.operator == BinaryOperator::Addition
                            && self.symbol(&sum.left) == Some(index)
                            && literal_number(&sum.right) == Some(3.0)
                ),
                _ => false,
            }
        })
    }

    /// The one alphabet symbol of the payload append inside the selected loop.
    fn payload_alphabet(
        &self,
        own: &Own<'a>,
        payload: SymbolId,
        index: SymbolId,
        bytes: SymbolId,
        loop_span: Span,
    ) -> Option<SymbolId> {
        own.assignments.iter().find_map(|assignment| {
            if !(loop_span.start <= assignment.span.start
                && assignment.span.end <= loop_span.end)
            {
                return None;
            }
            let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else {
                return None;
            };
            if analysis::identifier_symbol(target, self.scoping) != Some(payload) {
                return None;
            }
            let appended = match assignment.operator {
                AssignmentOperator::Assign => {
                    let Expression::BinaryExpression(sum) = strip(&assignment.right) else {
                        return None;
                    };
                    if sum.operator != BinaryOperator::Addition {
                        return None;
                    }
                    if self.symbol(&sum.left) == Some(payload) {
                        &sum.right
                    } else if self.symbol(&sum.right) == Some(payload) {
                        &sum.left
                    } else {
                        return None;
                    }
                }
                AssignmentOperator::Addition => &assignment.right,
                _ => return None,
            };
            let span = appended.span();
            let mut alphabet = None;
            let mut sextets = 0usize;
            let mut reads_bytes = false;
            for call in &own.calls {
                if call.span.start < span.start || call.span.end > span.end {
                    continue;
                }
                let Some((receiver, "charAt", arguments)) = method_call(call) else {
                    continue;
                };
                let Some(receiver) = self.symbol(receiver) else {
                    return None;
                };
                if alphabet.is_some_and(|known| known != receiver) {
                    return None;
                }
                alphabet = Some(receiver);
                sextets += 1;
                reads_bytes |= arguments
                    .first()
                    .is_some_and(|argument| self.references_byte_index(argument, bytes, index));
            }
            if sextets < 3 || !reads_bytes {
                return None;
            }
            alphabet
        })
    }

    /// Whether an expression reads `bytes[index]`, through loop-local aliases.
    fn references_byte_index(
        &self,
        expression: &Expression<'a>,
        bytes: SymbolId,
        index: SymbolId,
    ) -> bool {
        self.references_byte_index_at(expression, bytes, index, 0)
    }

    fn references_byte_index_at(
        &self,
        expression: &Expression<'a>,
        bytes: SymbolId,
        index: SymbolId,
        depth: usize,
    ) -> bool {
        if depth > 8 {
            return false;
        }
        match strip(expression) {
            Expression::ComputedMemberExpression(member) => {
                (self.symbol(&member.object) == Some(bytes)
                    && self.references_symbol(&member.expression, index))
                    || self.references_byte_index_at(&member.object, bytes, index, depth + 1)
                    || self.references_byte_index_at(&member.expression, bytes, index, depth + 1)
            }
            Expression::StaticMemberExpression(member) => {
                self.references_byte_index_at(&member.object, bytes, index, depth + 1)
            }
            Expression::BinaryExpression(binary) => {
                self.references_byte_index_at(&binary.left, bytes, index, depth + 1)
                    || self.references_byte_index_at(&binary.right, bytes, index, depth + 1)
            }
            Expression::UnaryExpression(unary) => {
                self.references_byte_index_at(&unary.argument, bytes, index, depth + 1)
            }
            Expression::CallExpression(call) => {
                self.references_byte_index_at(&call.callee, bytes, index, depth + 1)
                    || call_arguments(call).into_iter().any(|argument| {
                        self.references_byte_index_at(argument, bytes, index, depth + 1)
                    })
            }
            Expression::Identifier(identifier) => {
                let Some(symbol) = analysis::identifier_symbol(identifier, self.scoping) else {
                    return false;
                };
                let Some(value) = self.single_value(symbol) else {
                    return false;
                };
                self.references_byte_index_at(value, bytes, index, depth + 1)
            }
            _ => false,
        }
    }

    /// Whether an expression reads `symbol`.
    fn references_symbol(&self, expression: &Expression<'a>, symbol: SymbolId) -> bool {
        match strip(expression) {
            Expression::Identifier(identifier) => {
                analysis::identifier_symbol(identifier, self.scoping) == Some(symbol)
            }
            Expression::BinaryExpression(binary) => {
                self.references_symbol(&binary.left, symbol)
                    || self.references_symbol(&binary.right, symbol)
            }
            Expression::UnaryExpression(unary) => self.references_symbol(&unary.argument, symbol),
            _ => false,
        }
    }

    fn claim_envelope_state(
        &self,
        f: usize,
        writes: &[(&'a str, &'a Expression<'a>)],
        out: &mut Claims,
    ) {
        let own = self.own(f);
        for conditional in &own.conditionals {
            if member_of(&conditional.alternate).is_some_and(|(_, name)| name == "signalFilter") {
                if let Some(symbol) = self.outer_binding(&conditional.consequent, f) {
                    let is_array = self.single_value(symbol).is_some_and(
                        |value| matches!(strip(value), Expression::ArrayExpression(_)),
                    );
                    if is_array {
                        out.main(
                            DEFAULT_FILTER,
                            self.identifier(symbol),
                            &["array used when the envelope builder's signalFilter option is undefined"],
                        );
                    }
                }
            }
            if let Expression::BinaryExpression(test) = strip(&conditional.test) {
                let is_typeof = matches!(strip(&test.left), Expression::UnaryExpression(unary)
                    if unary.operator == UnaryOperator::Typeof);
                if is_typeof && string_value(&test.right) == Some("function") {
                    if let Some(symbol) = self.outer_binding(&conditional.alternate, f) {
                        out.main(
                            SIGNAL_ENCODER,
                            self.identifier(symbol),
                            &["encoder used when the envelope builder's encode option is not a function"],
                        );
                    }
                }
            }
        }
        for call in &own.calls {
            let Some((receiver, "toString", arguments)) = method_call(call) else {
                continue;
            };
            let Expression::CallExpression(hash) = strip(receiver) else {
                continue;
            };
            let radix = arguments
                .first()
                .is_some_and(|first| matches!(strip(first), Expression::NumericLiteral(_)));
            let hash_arguments = call_arguments(hash);
            let Some(Expression::BinaryExpression(sum)) = hash_arguments.first().map(|a| strip(a))
            else {
                continue;
            };
            if !radix || sum.operator != BinaryOperator::Addition {
                continue;
            }
            let Some(symbol) = self.outer_binding(&sum.left, f) else {
                continue;
            };
            if writes.iter().any(|(_, value)| self.outer_binding(value, f) == Some(symbol)) {
                out.main(
                    ENVELOPE_TOKEN,
                    self.identifier(symbol),
                    &["stored on the envelope and hashed with the encoded payload for the integrity field"],
                );
            }
        }
    }

    // -----------------------------------------------------------------------
    // Header helpers
    // -----------------------------------------------------------------------

    fn claim_chunker(&self, f: usize, out: &mut Claims) {
        let Some(result) = self.returned_null_object(f) else {
            return;
        };
        let (mut whole, mut first, mut rest) = (false, false, false);
        for assignment in &self.own(f).assignments {
            let AssignmentTarget::ComputedMemberExpression(target) = &assignment.left else {
                continue;
            };
            if self.symbol(&target.object) != Some(result) {
                continue;
            }
            let sliced = matches!(strip(&assignment.right), Expression::CallExpression(call)
                if method_call(call).is_some_and(|(receiver, name, arguments)| {
                    name == "slice" && self.is_slot(receiver, f, 1) && arguments.len() == 2
                }));
            if self.is_slot(&target.expression, f, 0) {
                let mut parts = Vec::new();
                additions(&assignment.right, &mut parts);
                whole |= matches!(parts.as_slice(), [text, value]
                    if string_value(text) == Some("") && self.is_slot(value, f, 1));
                first |= sliced;
            } else if let Expression::BinaryExpression(key) = strip(&target.expression) {
                rest |= sliced
                    && key.operator == BinaryOperator::Addition
                    && self.is_slot(&key.left, f, 0);
            }
        }
        if whole && first && rest && self.reaches_headers(f) {
            out.function(
                CHUNKER,
                self.fns[f].span,
                None,
                &[
                    "returns a null-prototype map of name to value, or of name, name0, name1... to size-limited slices",
                    "a caller writes request headers with headers.set",
                ],
            );
        }
    }

    /// Whether a function that calls `f` also calls `object.headers.set(...)`.
    fn reaches_headers(&self, f: usize) -> bool {
        self.fns.iter().any(|unit| {
            let calls_f =
                unit.own.calls.iter().any(|call| self.function_of(&call.callee) == Some(f));
            calls_f
                && unit.own.calls.iter().any(|call| {
                    method_call(call).is_some_and(|(receiver, name, _)| {
                        name == "set"
                            && member_of(receiver).is_some_and(|(_, field)| field == "headers")
                    })
                })
        })
    }

    fn claim_length_list(&self, f: usize, out: &mut Claims) {
        let Some(Expression::CallExpression(join)) = self.single_return(f).map(strip) else {
            return;
        };
        let Some((mapped, "join", separators)) = method_call(join) else {
            return;
        };
        let Expression::CallExpression(map) = strip(mapped) else {
            return;
        };
        let Some((keys, "map", callbacks)) = method_call(map) else {
            return;
        };
        let Expression::CallExpression(keys) = strip(keys) else {
            return;
        };
        let reads_keys = member_of(&keys.callee)
            .is_some_and(|(object, name)| name == "keys" && self.is_global(object, "Object"))
            && call_arguments(keys).first().is_some_and(|first| self.is_slot(first, f, 0));
        if !reads_keys || separators.first().and_then(|first| string_value(first)) != Some(" ") {
            return;
        }
        let Some(entry) = callbacks.first().and_then(|callback| self.function_of(callback)) else {
            return;
        };
        let Some(value) = self.single_return(entry) else {
            return;
        };
        let mut parts = Vec::new();
        additions(value, &mut parts);
        let formats = parts.iter().any(|part| self.is_slot(part, entry, 0))
            && parts.iter().any(|part| {
                member_of(part).is_some_and(|(object, name)| {
                    name == "length"
                        && matches!(strip(object), Expression::ComputedMemberExpression(element)
                            if self.is_slot(&element.object, f, 0)
                                && self.is_slot(&element.expression, entry, 0))
                })
            });
        if formats {
            out.function(
                LENGTH_LIST,
                self.fns[f].span,
                None,
                &["space-joins one entry per key of its object slot"],
            );
            out.function(
                LENGTH_ENTRY,
                self.fns[entry].span,
                None,
                &["map() callback; concatenates a key with the length of its value"],
            );
        }
    }
}
