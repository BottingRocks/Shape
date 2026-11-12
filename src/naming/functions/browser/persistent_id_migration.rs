//! Accepted legacy prefix grammar plus a separate whole-definition protocol role.
//!
//! The accepted042 path keeps its original priority, metadata and proof policy.
//! The new role consumes every own statement and nested continuation callback,
//! joining lexical SymbolIds without treating captured slots as stable objects.
//! Ports remain opaque/effectful; no helper, native API or runtime epoch fact is
//! exported. Only a private alphabet-counter loop is normalized in proof text.
use super::*;
use oxc_span::{GetSpan, Span};
use std::sync::OnceLock;

const NAME: &str = "migrateAndInitializePersistentId";
const EVALUATOR: &str = "persistent_id_migration";
// Definition-level role only. It is not an exported helper/algorithm contract.
const PROTOCOL_NAME: &str = "runPersistentIdMigrationProtocol";
pub(in crate::naming) const PROTOCOL_EVALUATOR: &str = "owned_persistent_id_migration_protocol_definition";
const CONTRACT: &str = include_str!("persistent_id_migration_contract.js");

// A single shared continuation grammar, not a body catalog. The helper calls
// stay opaque: this rule proves their receiver/arguments/order, not their purpose.
const CONTINUATION: &str = r#"
try {
    if (__pid2) {
        __pid14 = "l_" + __pid1.persistentId.key;
        __pid15 = "" + Date.now() + __pid16();
        if (!__pid2.getItem(__pid14)) {
            __pid2.setItem(__pid14, __pid15);
        }
    }
} catch (__pid17) {}
__pid18 = __pid19(__pid1.transactionToken);
__pid20();
try {
    __pid21.forEach(__pid22 => {
        __pid22.init(new __pid23(__pid24 => {
            let __pid25 = __pid24;
            __pid26.push(__pid27 => __pid25.next(__pid27));
        }));
        if (__pid22.afterReady != null) {
            if (typeof setTimeout !== "function") {
                __pid22.afterReady.call(void 0);
            } else if (__pid28) {
                setTimeout(__pid22.afterReady, 0);
            } else if (__pid29) {
                __pid29.push(__pid22.afterReady);
            }
        }
    });
} catch (__pid30) {
    let __pid31 = {};
    __pid31.message = "3.1:" + __pid30;
    __pid32(__pid31);
}
"#;

fn canonical_migration(source: &str) -> Option<String> {
    let mut code = canonical_compact(source)?;
    // Replacement requires the complete canonical AST of this private loop.
    // SymbolId renaming below proves the counter/table scope and use frontier.
    let alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_=";
    let target = format!("for (; __pid5 < \"{alphabet}\".length; __pid5++) {{ __pid4[\"{alphabet}\"[__pid5]] = __pid5; }}");
    let target = canonical_compact(&target)?;
    for update in ["++__pid5", "__pid5++"] {
        let lowered = format!("while (__pid5 < \"{alphabet}\".length) {{ __pid4[\"{alphabet}\"[__pid5]] = __pid5; {update}; }}");
        code = code.replace(&canonical_compact(&lowered)?, &target);
    }
    Some(code)
}

fn canonical(source: &str) -> Option<String> {
    canonical_options(source, false)
}

fn canonical_compact(source: &str) -> Option<String> {
    canonical_options(source, true)
}

fn canonical_options(source: &str, minify: bool) -> Option<String> {
    let allocator = Allocator::default();
    let program = crate::analysis::parse_program(&allocator, source).ok()?;
    Some(oxc_codegen::Codegen::new()
        .with_options(oxc_codegen::CodegenOptions {
            minify, comments: oxc_codegen::CommentOptions::disabled(),
            ..Default::default()
        })
        .build(&program).code)
}

fn captured_variable(model: &Model<'_, '_>, site: FunctionSite, symbol: SymbolId) -> bool {
    let declaration = model.scoping.symbol_declaration(symbol);
    !site.span.contains_inclusive(model.nodes.get_node(declaration).span())
        && model.scoping.symbol_redeclarations(symbol).is_empty()
        && matches!(model.nodes.kind(declaration), AstKind::VariableDeclarator(d)
            if matches!(&d.id, BindingPattern::BindingIdentifier(id)
                if id.symbol_id.get() == Some(symbol)))
}

// This classifies an opaque lexical slot, not an initialized or stable value.
// Outside writers, getters and called hooks may replace it between evaluations.
// No origin, native API, readiness, lifetime or purity fact is exported.
fn opaque_port_slot(model: &Model<'_, '_>, site: FunctionSite, symbol: SymbolId) -> bool {
    captured_variable(model, site, symbol)
}

fn normalized(model: &Model<'_, '_>, site: FunctionSite, span: Span) -> Option<String> {
    let mut identifiers = BTreeMap::new();
    for node in model.nodes.iter().filter(|n| span.contains_inclusive(n.kind().span())) {
        let (id_span, symbol) = match node.kind() {
            AstKind::BindingIdentifier(id) => (id.span, id.symbol_id.get()?),
            AstKind::IdentifierReference(id) => {
                let Some(symbol) = crate::analysis::identifier_symbol(id, model.scoping) else {
                    if !matches!(id.name.as_str(), "Error" | "Date" | "setTimeout") { return None; }
                    continue;
                };
                (id.span, symbol)
            }
            // Only the exact continuation arrows below are admitted; no donor functions.
            AstKind::Function(_) | AstKind::Class(_) | AstKind::WithStatement(_) => return None,
            _ => continue,
        };
        identifiers.insert((id_span.start, id_span.end), symbol);
    }
    let mut names = BTreeMap::new();
    let mut edits = Vec::new();
    for ((start, end), symbol) in identifiers {
        let next = names.len();
        let index = *names.entry(symbol).or_insert(next);
        if matches!(index, 1 | 2 | 8 | 14 | 15 | 16 | 18 | 19 | 20 | 21 | 23 | 26 | 28 | 29 | 32) {
            if !opaque_port_slot(model, site, symbol) { return None; }
        } else {
            let declaration = model.scoping.symbol_declaration(symbol);
            if !span.contains_inclusive(model.nodes.get_node(declaration).span())
                || !model.scoping.symbol_redeclarations(symbol).is_empty()
                || model.scoping.get_resolved_references(symbol).any(|r|
                    r.is_write() && !span.contains_inclusive(model.nodes.get_node(r.node_id()).span())) {
                return None;
            }
        }
        edits.push((start, end, format!("__pid{index}")));
    }
    if names.len() != 33 { return None; }

    let mut source = model.source.get(span.start as usize..span.end as usize)?.to_string();
    for (start, end, replacement) in edits.into_iter().rev() {
        source.replace_range((start-span.start) as usize..(end-span.start) as usize, &replacement);
    }
    canonical_migration(&source)
}

fn legacy_normalized(model: &Model<'_, '_>, site: FunctionSite, span: Span) -> Option<String> {
    let mut identifiers = BTreeMap::new();
    for node in model.nodes.iter().filter(|n| span.contains_inclusive(n.kind().span())) {
        let (id_span, symbol) = match node.kind() {
            AstKind::BindingIdentifier(id) => (id.span, id.symbol_id.get()?),
            AstKind::IdentifierReference(id) => {
                let Some(symbol) = crate::analysis::identifier_symbol(id, model.scoping) else {
                    if id.name != "Error" { return None; }
                    continue;
                };
                (id.span, symbol)
            }
            // No nested body, dynamic scope or opaque donor is admitted.
            AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)
            | AstKind::Class(_) | AstKind::WithStatement(_) => return None,
            _ => continue,
        };
        identifiers.insert((id_span.start, id_span.end), symbol);
    }
    let mut names = BTreeMap::new();
    let mut edits = Vec::new();
    for ((start, end), symbol) in identifiers {
        let next = names.len();
        let index = *names.entry(symbol).or_insert(next);
        if matches!(index, 1 | 2 | 8) {
            if !captured_variable(model, site, symbol) { return None; }
            // Config/storage must not be replaced by a local write in this body.
            if index != 8 && model.scoping.get_resolved_references(symbol).any(|r|
                r.is_write() && site.span.contains_inclusive(model.nodes.get_node(r.node_id()).span())) {
                return None;
            }
        } else {
            let declaration = model.scoping.symbol_declaration(symbol);
            if !span.contains_inclusive(model.nodes.get_node(declaration).span())
                || !model.scoping.symbol_redeclarations(symbol).is_empty()
                || model.scoping.get_resolved_references(symbol).any(|r|
                    r.is_write() && !span.contains_inclusive(model.nodes.get_node(r.node_id()).span())) {
                return None;
            }
        }
        edits.push((start, end, format!("__pid{index}")));
    }
    if names.len() != 14 { return None; }
    let mut source = model.source.get(span.start as usize..span.end as usize)?.to_string();
    for (start, end, replacement) in edits.into_iter().rev() {
        source.replace_range((start-span.start) as usize..(end-span.start) as usize, &replacement);
    }
    canonical(&source)
}


pub(in crate::naming) fn evaluate<'a>(
    model: &Model<'_, 'a>, site: FunctionSite, f: &Facts<'_>,
    _proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else { return Vec::new(); };
    if function.r#async || function.generator || function.params.rest.is_some()
        || !function.params.items.is_empty() { return Vec::new(); }
    let Some(body) = &function.body else { return Vec::new(); };
    if !body.directives.is_empty() || body.statements.len() < 5 { return Vec::new(); }
    // Preserve the accepted rule exactly; the extension below has its own
    // complete-body proof and cannot borrow the legacy prefix's acceptance.
    let legacy_span = Span::new(body.statements[0].span().start, body.statements[4].span().end);
    static LEGACY_EXPECTED: OnceLock<String> = OnceLock::new();
    let legacy_expected = LEGACY_EXPECTED.get_or_init(|| canonical(CONTRACT).expect("migration grammar parses"));
    if legacy_span.size() <= 16000
        && model.source[legacy_span.start as usize..legacy_span.end as usize].contains("persistentId")
        && legacy_normalized(model, site, legacy_span).as_ref() == Some(legacy_expected) {
        return vec![proposal(NAME, EVALUATOR,
            "exact owned lexical migration grammar: captured storage/config; URL-safe legacy f decode, padding and version validation; same-value migration under persistentId.key; resetKey removal and transactionToken initialization on that same storage/config; no nested or adjacent evidence", f)];
    }
    if body.statements.len() != 9 { return Vec::new(); }
    let prefix = Span::new(body.statements[0].span().start, body.statements[8].span().end);
    // Cheap necessary prefilter only. The entire grammar below establishes proof.
    if prefix.size() > 16000 || !model.source[prefix.start as usize..prefix.end as usize]
        .contains("persistentId") { return Vec::new(); }
    static EXPECTED: OnceLock<String> = OnceLock::new();
    let expected = EXPECTED.get_or_init(|| canonical_migration(&format!("{CONTRACT}\n{CONTINUATION}")).expect("migration grammar parses"));
    if normalized(model, site, prefix).as_ref() != Some(expected) { return Vec::new(); }
    vec![proposal(PROTOCOL_NAME, PROTOCOL_EVALUATOR,
        "complete owned protocol definition: local URL-safe legacy decode/padding/version guards; ordered migration, reset and fallback requests; lease/token/module/observer/afterReady/error continuations with exact scoped reads, receivers, arguments and exception order; every external port read/call is opaque and effectful, repeated lexical slots need not retain object values; no entry readiness, native API/helper algorithm identity, stable backend, entropy quality, persistence success or callback delivery is established", f)]
}
