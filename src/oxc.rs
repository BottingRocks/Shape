//! Shared Oxc parsing, traversal, code generation, and AST analysis helpers.
//!
//! Every stage parses its input, transforms the Oxc AST (or an exact-span edit
//! list derived from it), and re-emits code with Oxc's codegen. No JavaScript
//! parser subprocess, Babel, Node or npm is involved. The implementation path is
//! `oxc`; the crate root preserves the public `shape::analysis` alias.
//! VM-specific nested-dispatch recognition belongs to [`crate::vm_patterns`],
//! not to these generic helpers.
//!
//! # Pass lifecycle
//!
//! 1. Allocate an Oxc arena and call [`parse`] on the exact source being
//!    examined. Check both `errors` and `panicked` before visiting the program.
//! 2. Call [`analyze`] to obtain scoping and AST nodes. Resolve references
//!    through that scoping when deciding whether two uses refer to the same
//!    binding.
//! 3. Either transform the AST and call [`codegen`], or collect exact source
//!    edits and call [`apply_edits`]. The two outputs have different coordinate
//!    implications.
//! 4. If another pass needs semantic facts after a rewrite, parse and analyze
//!    the new source. Old spans and symbol IDs belong to the earlier parse.
//!
//! # Binding and edit contracts
//!
//! [`symbol_reference_spans`] returns references only; [`symbol_all_spans`] also
//! includes the declaration identifier. [`reference_symbol`] can return `None`
//! for an unresolved reference. A familiar global spelling or a list entry in
//! [`NATIVE_GLOBALS`] does not establish that the global is unshadowed or
//! stable; those are proof obligations for the consumer.
//!
//! [`Edit::replace`] and [`Edit::delete`] use UTF-8 byte offsets into the
//! supplied source. Callers must supply character-boundary offsets from that
//! source's AST. [`apply_edits`] sorts edits, coalesces identical nonempty-range
//! replacements, and returns overlaps or out-of-range edits in
//! [`AppliedEdits::rejected`]. It can apply some edits while rejecting others: a
//! caller requiring an all-or-nothing rewrite must check `rejected` before
//! adopting `output`. A codegen pass can change all printed positions, so old
//! source maps cannot simply be carried forward.
//!
//! `tests/analysis_unary_guard.rs` covers the parser guards and
//! `tests/apply_edits.rs` covers edit ordering, duplicates, and rejection.

use oxc_allocator::Allocator;
use oxc_ast::ast::{Expression, IdentifierReference, Program};
use oxc_ast_visit::{walk, Visit};
use oxc_codegen::{Codegen, CodegenOptions, IndentChar};
use oxc_parser::{ParseOptions, Parser};
use oxc_semantic::{AstNodes, Scoping, SemanticBuilder, SymbolId};
use oxc_span::{GetSpan, SourceType, Span};

// ===== Parsing =====

// Source parsing with pre-parser depth guards.
//
// Oxc's parser recurses over the AST it builds. Two adversarial shapes (a long
// chain of unary prefix operators, or deeply nested brackets) can overflow the
// process stack before the parser produces an AST, so a bounded lexical scan
// rejects them with a typed diagnostic instead. The scan is not a source-size
// limit; it only classifies the operator/bracket shape.

/// Recursive Oxc AST consumers in this crate are deliberately kept below a
/// conservative depth.  The guard is iterative in the sense that it stops
/// descending before the AST's recursive expression shape can reach the
/// process stack limit; ordinary shallow programs still use Oxc's visitors.
pub(crate) const MAX_EXPRESSION_DEPTH: usize = 256;

/// Bracket nesting (`(`, `[`, `{`, `${`) accepted before Oxc's recursive parser
/// is entered. Deeper sources are rejected with a typed diagnostic instead of
/// overflowing the process stack.
pub(crate) const MAX_NESTING_DEPTH: usize = 1024;

/// Result of parsing a source string.
pub struct ParseOutput<'a> {
    pub program: Program<'a>,
    pub errors: Vec<String>,
    pub panicked: bool,
}

thread_local! {
    static OWN_OUTPUT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// While the guard lives, parses on this thread read the program's own
/// cleanup output: code it printed from input that already passed the
/// source-depth scan, run on the large-stack thread. They skip that scan
/// (about a third of a parse); the expression-depth guard still runs.
pub struct OwnOutput(bool);

impl OwnOutput {
    pub fn enter() -> Self {
        Self(OWN_OUTPUT.with(|own| own.replace(true)))
    }
}

impl Drop for OwnOutput {
    fn drop(&mut self) {
        OWN_OUTPUT.with(|own| own.set(self.0));
    }
}

/// Parse with parenthesised expressions removed, so the AST is canonical.
pub fn parse<'a>(allocator: &'a Allocator, source: &'a str) -> ParseOutput<'a> {
    let violation = if OWN_OUTPUT.with(std::cell::Cell::get) { None } else { excessive_source_depth(source) };
    if let Some(violation) = violation {
        // Oxc's parser is intentionally not entered for adversarial shapes
        // known to recurse before it can produce an AST.  Parse an empty
        // fallback program only to satisfy this API's owned-program contract;
        // callers already treat the typed diagnostic as a failed parse.
        let fallback = Parser::new(allocator, "", SourceType::unambiguous())
            .with_options(ParseOptions {
                preserve_parens: false,
                ..ParseOptions::default()
            })
            .parse();
        return ParseOutput {
            program: fallback.program,
            errors: vec![match violation {
                DepthViolation::Unary(depth) => format!(
                    "source unary expression depth exceeds configured limit {MAX_EXPRESSION_DEPTH} (observed at least {depth})"
                ),
                DepthViolation::Nesting(depth) => format!(
                    "source bracket nesting depth exceeds configured limit {MAX_NESTING_DEPTH} (observed at least {depth})"
                ),
            }],
            panicked: false,
        };
    }
    let __profile = crate::profile::start();
    let ret = Parser::new(allocator, source, SourceType::unambiguous())
        .with_options(ParseOptions {
            preserve_parens: false,
            ..ParseOptions::default()
        })
        .parse();
    crate::profile::done("oxc.parse", __profile);
    let program = ret.program;
    let mut errors = ret
        .errors
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let mut depth = ExpressionDepthGuard {
        depth: 0,
        exceeded: None,
    };
    depth.visit_program(&program);
    if let Some(span) = depth.exceeded {
        errors.push(format!(
            "source expression depth exceeds configured limit {MAX_EXPRESSION_DEPTH} at {}..{}",
            span.start, span.end
        ));
    }
    ParseOutput {
        program,
        errors,
        panicked: ret.panicked,
    }
}

/// Parse a source string and retain the historical joined parse diagnostic.
pub fn parse_program<'a>(
    allocator: &'a Allocator,
    source: &'a str,
) -> Result<Program<'a>, String> {
    let parsed = parse(allocator, source);
    if parsed.panicked || !parsed.errors.is_empty() {
        Err(parsed.errors.join("; "))
    } else {
        Ok(parsed.program)
    }
}

/// Check that a pass's edited output still parses. The next pass parses it
/// anyway, so outside debug builds the check only runs with
/// `SHAPE_CHECK_EDITS` set (to name the pass that broke it); the program
/// command parses its final output once.
pub(crate) fn check_program<'a>(allocator: &'a Allocator, source: &'a str) -> Result<(), String> {
    if !cfg!(debug_assertions) && std::env::var_os("SHAPE_CHECK_EDITS").is_none() {
        return Ok(());
    }
    parse_program(allocator, source).map(|_| ())
}

/// A source shape whose depth would make Oxc's recursive parser abort before
/// it can produce an AST.
enum DepthViolation {
    /// A chain of unary prefix operators (`!`, `~`, `+`, `-`).
    Unary(usize),
    /// Nested `(`, `[`, `{` or template `${`.
    Nesting(usize),
}

/// Detect a chained unary prefix (`!`, `~`, `+`, `-`) or a bracket nesting
/// whose depth would make Oxc's recursive parser abort before it can produce
/// an AST.
///
/// This is a bounded lexical scan, not a general JavaScript lexer. It skips
/// comments, string literals, template text and regular-expression literals so
/// that text such as `/!!!!!/` or `"!!!!!"` is not mistaken for code, while a
/// genuine chain of code operators is still detected. Template `${...}`
/// interpolation is treated as code; comments and whitespace do not break a
/// chain, but any other code token does. `!=`/`!==` are comparisons, not unary
/// operators, and `+`/`-` count as unary only where an operand is expected. The result is a shape check for the parser-overflow case, not a
/// source-size or character-count limit.
fn excessive_source_depth(source: &str) -> Option<DepthViolation> {
    let bytes = source.as_bytes();
    let len = bytes.len();
    let mut index = 0usize;
    let mut depth = 0usize;
    // Open brackets in code, including template interpolations.
    let mut nesting = 0usize;
    // Whether a `/` at the cursor is division (the previous significant token
    // can end an expression) rather than the start of a regex literal.
    let mut division_allowed = false;
    // Nesting contexts: template text versus interpolated code. A code frame
    // carries the brace depth used to find the `}` that closes its `${`.
    let mut contexts: Vec<(ScanCtx, usize)> = vec![(ScanCtx::Code, 0)];
    // True immediately after a control-flow keyword, so the next `(` is read as
    // a statement header rather than a call or grouping.
    let mut control_keyword_just_seen = false;
    // Parenthesis kinds: a control-header `)` starts a statement and allows a
    // following regex, an ordinary call/group `)` allows division.
    let mut parens: Vec<ParenKind> = Vec::new();
    // True while the next identifier is a member-access property name, so it is
    // never read as a control-flow keyword (`obj.if(...)` is an ordinary call).
    let mut after_dot = false;

    while index < len {
        if matches!(contexts.last(), Some((ScanCtx::Template, _))) {
            match bytes[index] {
                b'\\' => index = (index + 2).min(len),
                b'`' => {
                    contexts.pop();
                    index += 1;
                    depth = 0;
                    if matches!(contexts.last(), Some((ScanCtx::Code, _))) {
                        division_allowed = true;
                    }
                }
                b'$' if bytes.get(index + 1) == Some(&b'{') => {
                    nesting += 1;
                    if nesting > MAX_NESTING_DEPTH {
                        return Some(DepthViolation::Nesting(nesting));
                    }
                    contexts.push((ScanCtx::Code, 0));
                    index += 2;
                    depth = 0;
                    division_allowed = false;
                }
                _ => {
                    index += 1;
                    depth = 0;
                }
            }
            continue;
        }

        let byte = bytes[index];
        // Trivia never breaks a unary chain and never changes the
        // regex-vs-division context.
        match byte {
            b' ' | b'\t' | b'\r' | b'\n' | 0x0b | 0x0c => {
                index += 1;
                continue;
            }
            b'/' if bytes.get(index + 1) == Some(&b'/') => {
                index += 2;
                while index < len && bytes[index] != b'\n' {
                    index += 1;
                }
                continue;
            }
            b'/' if bytes.get(index + 1) == Some(&b'*') => {
                index += 2;
                while index + 1 < len && !(bytes[index] == b'*' && bytes[index + 1] == b'/') {
                    index += 1;
                }
                index = (index + 2).min(len);
                continue;
            }
            _ => {}
        }
        if is_identifier_byte(byte) && !byte.is_ascii_digit() {
            // Consume the token in one tight scan. Linked programs contain long
            // generated identifiers; revisiting the context/trivia machinery
            // and pushing to a String for every byte dominated this guard.
            let start = index;
            index += 1;
            while index < len && is_identifier_byte(bytes[index]) {
                index += 1;
            }
            let word = &source[start..index];
            if after_dot {
                // Property names are never control keywords.
                division_allowed = true;
                control_keyword_just_seen = false;
            } else {
                division_allowed = word_ends_expression(word);
                control_keyword_just_seen = is_control_keyword(word);
            }
            after_dot = false;
            continue;
        }
        if byte != b'(' {
            control_keyword_just_seen = false;
        }

        match byte {
            b'.' => {
                after_dot = true;
                index += 1;
                depth = 0;
                division_allowed = false;
            }
            b'/' => {
                if division_allowed {
                    index += 1;
                    depth = 0;
                    division_allowed = false;
                } else {
                    index = scan_regex_literal(bytes, index);
                    depth = 0;
                    division_allowed = true;
                }
            }
            b'!' => {
                if bytes.get(index + 1) == Some(&b'=') {
                    index += if bytes.get(index + 2) == Some(&b'=') {
                        3
                    } else {
                        2
                    };
                    depth = 0;
                    division_allowed = false;
                } else {
                    depth += 1;
                    if depth > MAX_EXPRESSION_DEPTH {
                        return Some(DepthViolation::Unary(depth));
                    }
                    index += 1;
                    division_allowed = false;
                }
            }
            b'~' => {
                depth += 1;
                if depth > MAX_EXPRESSION_DEPTH {
                    return Some(DepthViolation::Unary(depth));
                }
                index += 1;
                division_allowed = false;
            }
            b'+' if bytes.get(index + 1) == Some(&b'+') => {
                // Postfix `a++` keeps the preceding "expression ended" context so
                // a following `/` is division; prefix `++a` is followed by its
                // operand, which sets the context itself.
                index += 2;
                depth = 0;
            }
            b'-' if bytes.get(index + 1) == Some(&b'-') => {
                index += 2;
                depth = 0;
            }
            b'+' | b'-' if !division_allowed && bytes.get(index + 1) != Some(&b'=') => {
                // No operand precedes it, so this is a unary prefix.
                depth += 1;
                if depth > MAX_EXPRESSION_DEPTH {
                    return Some(DepthViolation::Unary(depth));
                }
                index += 1;
            }
            b'\'' | b'"' => {
                index = scan_quoted_literal(bytes, index);
                depth = 0;
                division_allowed = true;
            }
            b'`' => {
                contexts.push((ScanCtx::Template, 0));
                index += 1;
                depth = 0;
            }
            b'0'..=b'9' => {
                index = scan_number_literal(bytes, index);
                depth = 0;
                division_allowed = true;
            }
            b'(' => {
                nesting += 1;
                if nesting > MAX_NESTING_DEPTH {
                    return Some(DepthViolation::Nesting(nesting));
                }
                parens.push(if control_keyword_just_seen {
                    ParenKind::ControlHeader
                } else {
                    ParenKind::Ordinary
                });
                control_keyword_just_seen = false;
                index += 1;
                depth = 0;
                division_allowed = false;
            }
            b')' => {
                // A control-header `)` starts a statement (regex allowed); an
                // ordinary call/group `)` ends an expression (division allowed).
                let kind = parens.pop().unwrap_or(ParenKind::Ordinary);
                nesting = nesting.saturating_sub(1);
                index += 1;
                depth = 0;
                division_allowed = matches!(kind, ParenKind::Ordinary);
            }
            b'[' => {
                nesting += 1;
                if nesting > MAX_NESTING_DEPTH {
                    return Some(DepthViolation::Nesting(nesting));
                }
                index += 1;
                depth = 0;
                division_allowed = false;
            }
            b']' => {
                nesting = nesting.saturating_sub(1);
                index += 1;
                depth = 0;
                division_allowed = true;
            }
            b'}' => {
                nesting = nesting.saturating_sub(1);
                let closes_interpolation = contexts.last().is_some_and(|(_, brace)| *brace == 0)
                    && contexts.len() >= 2
                    && matches!(contexts[contexts.len() - 2], (ScanCtx::Template, _));
                if closes_interpolation {
                    contexts.pop();
                    index += 1;
                    depth = 0;
                    division_allowed = true;
                } else {
                    if let Some((_, brace)) = contexts.last_mut() {
                        *brace = brace.saturating_sub(1);
                    }
                    index += 1;
                    depth = 0;
                    division_allowed = false;
                }
            }
            b'{' => {
                nesting += 1;
                if nesting > MAX_NESTING_DEPTH {
                    return Some(DepthViolation::Nesting(nesting));
                }
                if let Some((_, brace)) = contexts.last_mut() {
                    *brace += 1;
                }
                index += 1;
                depth = 0;
                division_allowed = false;
            }
            _ => {
                index += 1;
                depth = 0;
                division_allowed = false;
            }
        }
    }
    None
}

#[derive(Clone, Copy)]
enum ScanCtx {
    Code,
    Template,
}

#[derive(Clone, Copy)]
enum ParenKind {
    /// The `(` of an `if`/`while`/`for`/`with`/`switch`/`catch` header. Its
    /// matching `)` starts a statement, so a following `/` is a regex.
    ControlHeader,
    /// Any other `(`, e.g. a call or grouping. Its matching `)` ends an
    /// expression, so a following `/` is division.
    Ordinary,
}

fn is_identifier_byte(byte: u8) -> bool {
    byte == b'_' || byte == b'$' || byte.is_ascii_alphanumeric()
}

/// Keywords whose parenthesised header is followed by a statement.
fn is_control_keyword(word: &str) -> bool {
    matches!(word, "if" | "while" | "for" | "with" | "switch" | "catch")
}

/// Classify an identifier token for the regex-vs-division decision: `true` when
/// the token can end an expression, so a following `/` is division.
fn word_ends_expression(word: &str) -> bool {
    !matches!(
        word,
        "return"
            | "typeof"
            | "delete"
            | "void"
            | "new"
            | "in"
            | "instanceof"
            | "throw"
            | "case"
            | "do"
            | "else"
            | "yield"
            | "await"
            | "of"
            | "default"
            | "if"
            | "while"
            | "for"
            | "with"
            | "switch"
            | "catch"
    )
}

fn scan_quoted_literal(bytes: &[u8], start: usize) -> usize {
    let len = bytes.len();
    let quote = bytes[start];
    let mut index = start + 1;
    while index < len {
        match bytes[index] {
            b'\\' => index = (index + 2).min(len),
            byte if byte == quote => {
                index += 1;
                break;
            }
            b'\n' => break,
            _ => index += 1,
        }
    }
    index
}

fn scan_number_literal(bytes: &[u8], start: usize) -> usize {
    let len = bytes.len();
    let mut index = start;
    while index < len {
        let byte = bytes[index];
        if byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'_' {
            index += 1;
        } else {
            break;
        }
    }
    index
}

/// Scan a regex literal beginning at `start` (a `/`), honoring escapes and
/// character classes, then consume its trailing flags.
fn scan_regex_literal(bytes: &[u8], start: usize) -> usize {
    let len = bytes.len();
    let mut index = start + 1;
    let mut in_class = false;
    while index < len {
        match bytes[index] {
            b'\\' => index = (index + 2).min(len),
            b'[' => {
                in_class = true;
                index += 1;
            }
            b']' => {
                in_class = false;
                index += 1;
            }
            b'/' if !in_class => {
                index += 1;
                break;
            }
            b'\n' => break,
            _ => index += 1,
        }
    }
    while index < len && is_identifier_byte(bytes[index]) {
        index += 1;
    }
    index
}

struct ExpressionDepthGuard {
    depth: usize,
    exceeded: Option<Span>,
}

impl<'a> Visit<'a> for ExpressionDepthGuard {
    fn visit_expression(&mut self, expression: &oxc_ast::ast::Expression<'a>) {
        if self.depth >= MAX_EXPRESSION_DEPTH {
            self.exceeded.get_or_insert(expression.span());
            return;
        }
        self.depth += 1;
        walk::walk_expression(self, expression);
        self.depth -= 1;
    }
}

// ===== Semantics =====

// Semantic analysis and binding/scope helpers.
//
// `analyze` runs Oxc's semantic builder once and returns the scope tree plus
// node table. The remaining helpers resolve identifier references to their
// lexical symbols and collect the spans a symbol occupies, so downstream
// stages can make decisions from binding identity rather than spelling.

/// Run semantic analysis and return the owned scope tree plus node table.
pub fn analyze<'a>(program: &'a Program<'a>) -> (Scoping, AstNodes<'a>) {
    let __profile = crate::profile::start();
    let semantic = SemanticBuilder::new()
        .with_check_syntax_error(false)
        .build(program)
        .semantic
        .into_scoping_and_nodes();
    crate::profile::done("oxc.analyze", __profile);
    semantic
}

/// Resolve an Oxc identifier reference to its lexical symbol.
pub(crate) fn identifier_symbol(
    identifier: &IdentifierReference<'_>,
    scoping: &Scoping,
) -> Option<SymbolId> {
    identifier
        .reference_id
        .get()
        .and_then(|reference| scoping.get_reference(reference).symbol_id())
}

/// Resolve an identifier expression to its lexical symbol.
pub(crate) fn expression_symbol(
    expression: &Expression<'_>,
    scoping: &Scoping,
) -> Option<SymbolId> {
    let Expression::Identifier(identifier) = expression else {
        return None;
    };
    identifier_symbol(identifier, scoping)
}

/// All identifier spans that resolve to a given symbol (references only; the
/// declaration span is obtained separately from the scope tree).
pub fn symbol_reference_spans(
    scoping: &Scoping,
    nodes: &AstNodes,
    symbol: oxc_semantic::SymbolId,
) -> Vec<Span> {
    use oxc_ast::AstKind;
    let mut spans = Vec::new();
    for reference in scoping.get_resolved_references(symbol) {
        if let AstKind::IdentifierReference(id) = nodes.kind(reference.node_id()) {
            spans.push(id.span);
        }
    }
    spans
}

/// The full set of spans occupied by a symbol: its declaration identifier plus
/// every resolved reference.
pub fn symbol_all_spans(
    scoping: &Scoping,
    nodes: &AstNodes,
    symbol: oxc_semantic::SymbolId,
) -> Vec<Span> {
    let mut spans = vec![scoping.symbol_span(symbol)];
    spans.extend(symbol_reference_spans(scoping, nodes, symbol));
    spans
}

/// Does this identifier reference resolve to a binding (as opposed to an
/// unresolved global)?
pub fn reference_symbol(
    scoping: &Scoping,
    reference: oxc_semantic::ReferenceId,
) -> Option<oxc_semantic::SymbolId> {
    scoping.get_reference(reference).symbol_id()
}

// ===== Code generation =====

// Deterministic JavaScript code generation.
//
// Every stage re-emits transformed ASTs through this single codegen entry
// point so output formatting stays identical across passes.

/// Deterministic, readable code generation (two-space indent, double quotes).
pub fn codegen(program: &Program) -> String {
    let __profile = crate::profile::start();
    let code = Codegen::new()
        .with_options(CodegenOptions {
            indent_char: IndentChar::Space,
            indent_width: 2,
            ..CodegenOptions::default()
        })
        .build(program)
        .code;
    crate::profile::done("oxc.codegen", __profile);
    code
}

// ===== Source edits =====

// Exact-byte-range source edits.
//
// Several passes describe their output as a list of non-overlapping,
// exact-span replacements rather than re-printing the AST. `apply_edits` is
// the one splice: overlapping or out-of-range edits are not applied and are
// reported back so callers can decline instead of emitting a silent partial
// rewrite.

/// A single exact-byte-range replacement in a source string. An empty
/// replacement deletes the range.
#[derive(Debug, Clone)]
pub struct Edit {
    pub start: u32,
    pub end: u32,
    pub replacement: String,
}

impl Edit {
    pub fn replace(span: Span, replacement: impl Into<String>) -> Self {
        Self {
            start: span.start,
            end: span.end,
            replacement: replacement.into(),
        }
    }

    pub fn delete(span: Span) -> Self {
        Self {
            start: span.start,
            end: span.end,
            replacement: String::new(),
        }
    }
}

/// Result of [`apply_edits`].
#[derive(Debug)]
pub struct AppliedEdits {
    pub output: String,
    /// Edits written into `output`. Exact duplicates are counted once.
    pub applied: usize,
    /// Edits that overlapped an earlier edit or fell outside `source`. They
    /// were not applied, so callers must not report them as changes.
    pub rejected: Vec<Edit>,
}

/// Apply edits to `source`. Overlapping or out-of-range edits are not applied;
/// they are returned in [`AppliedEdits::rejected`] so the caller can decline
/// or report a partial rewrite instead of emitting it silently.
pub fn apply_edits(source: &str, mut edits: Vec<Edit>) -> AppliedEdits {
    edits.sort_by(|a, b| a.start.cmp(&b.start).then(a.end.cmp(&b.end)));
    let len = source.len() as u32;
    let mut out = String::with_capacity(source.len());
    let mut cursor: u32 = 0;
    let mut applied = 0;
    let mut rejected = Vec::new();
    let mut previous: Option<Edit> = None;
    for e in edits {
        let duplicate = previous.as_ref().is_some_and(|previous| {
            previous.start == e.start
                && previous.end == e.end
                && previous.start != previous.end
                && previous.replacement == e.replacement
        });
        if duplicate {
            continue;
        }
        if e.start < cursor || e.end < e.start || e.end > len {
            rejected.push(e);
            continue;
        }
        out.push_str(&source[cursor as usize..e.start as usize]);
        out.push_str(&e.replacement);
        cursor = e.end;
        applied += 1;
        previous = Some(e);
    }
    out.push_str(&source[cursor as usize..]);
    AppliedEdits {
        output: out,
        applied,
        rejected,
    }
}

/// One-line description of edits that [`apply_edits`] did not apply.
pub fn describe_rejected_edits(rejected: &[Edit]) -> String {
    let spans = rejected
        .iter()
        .take(5)
        .map(|edit| format!("{}..{}", edit.start, edit.end))
        .collect::<Vec<_>>()
        .join(", ");
    let more = rejected.len().saturating_sub(5);
    if more > 0 {
        format!(
            "{} overlapping edit(s) not applied: {spans}, and {more} more",
            rejected.len()
        )
    } else {
        format!(
            "{} overlapping edit(s) not applied: {spans}",
            rejected.len()
        )
    }
}

// ===== JavaScript globals and names =====

// JavaScript global and identifier-name helpers.
//
// These are the shared spellings several passes use to decide whether a name
// refers to a well-known built-in global and whether a string is a valid
// ECMAScript identifier name (for `obj["name"]` -> `obj.name`).

/// Built-in globals that a conservative native-alias resolver may inline.
pub const NATIVE_GLOBALS: &[&str] = &[
    "ReferenceError",
    "TypeError",
    "RangeError",
    "SyntaxError",
    "URIError",
    "EvalError",
    "Error",
    "AggregateError",
    "Object",
    "Function",
    "Array",
    "RegExp",
    "Number",
    "String",
    "Boolean",
    "Symbol",
    "BigInt",
    "Math",
    "JSON",
    "Reflect",
    "Proxy",
    "Promise",
    "Date",
    "Map",
    "Set",
    "WeakMap",
    "WeakSet",
    "ArrayBuffer",
    "SharedArrayBuffer",
    "DataView",
    "Uint8Array",
    "Uint8ClampedArray",
    "Int8Array",
    "Uint16Array",
    "Int16Array",
    "Uint32Array",
    "Int32Array",
    "Float32Array",
    "Float64Array",
    "BigInt64Array",
    "BigUint64Array",
    "TextDecoder",
    "TextEncoder",
    "URL",
    "Blob",
    "Worker",
    "WebAssembly",
    "globalThis",
    "window",
    "self",
    "document",
    "navigator",
    "parseFloat",
    "parseInt",
    "isNaN",
    "isFinite",
    "decodeURIComponent",
    "encodeURIComponent",
    "decodeURI",
    "encodeURI",
    "setTimeout",
    "setInterval",
    "clearTimeout",
    "clearInterval",
    "atob",
    "btoa",
    "console",
    "crypto",
    "performance",
];

/// True when `name` is a well-known JavaScript global.
pub fn is_native_global(name: &str) -> bool {
    NATIVE_GLOBALS.contains(&name)
}

/// True when `name` is a valid ECMAScript identifier name (used to decide
/// whether `obj["name"]` can become `obj.name`).
pub fn is_identifier_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_alphabetic() || first == '_' || first == '$') {
        return false;
    }
    chars.all(|c| c.is_alphanumeric() || c == '_' || c == '$')
}

// ===== Spans =====

// Small span helpers shared by diagnostics and reports.

/// Description of a span used in diagnostics.
pub fn span_label(span: Span) -> String {
    format!("{}..{}", span.start, span.end)
}

/// Byte length helper that avoids pulling `GetSpan` into call sites.
pub fn span_len(node: &impl GetSpan) -> u32 {
    node.span().size()
}
