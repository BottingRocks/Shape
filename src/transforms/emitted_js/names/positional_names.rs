//! Name every generated binding by where it is declared: `$A`, `$B`, …
//!
//! Whole-program stage 13d of `trace program`. Each function numbers the generated
//! bindings it owns (block-scoped ones included) in source order, `$A` to
//! `$Z`, then `$AA`, `$AB`, …. A function skips every name its enclosing
//! functions use, so no name shadows another: along any chain of scopes a
//! name is one binding (sibling functions reuse names). Thread functions keep their
//! `$THREAD_` names and the slots threads share keep their `$KEY_` names;
//! hand-written names are left alone.
//!
//! Renaming is by resolved symbol: the declaration and every reference move
//! together. A candidate that already occurs in the source as a `$`-name is
//! skipped. `with`/`eval` disables the pass.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{ScopeId, Scoping, SymbolId};
use oxc_span::GetSpan;

use crate::analysis;

pub(crate) fn name_bindings_by_position(source: &str) -> Result<(String, usize), String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let mut dynamic = Dynamic::default();
    dynamic.visit_program(&program);
    if dynamic.found {
        return Ok((source.into(), 0));
    }
    let (scoping, nodes) = analysis::analyze(&program);
    let taken = dollar_names(source);

    // The generated bindings, by the function that owns them.
    let mut owned: BTreeMap<ScopeId, Vec<SymbolId>> = BTreeMap::new();
    let mut owner = HashMap::new();
    for symbol in scoping.symbol_ids() {
        if !generated(scoping.symbol_name(symbol)) {
            continue;
        }
        let function = function_of(&scoping, scoping.symbol_scope_id(symbol));
        owned.entry(function).or_default().push(symbol);
        owner.insert(symbol, function);
    }
    // Scope ids follow source order, so outer functions are named first.
    // A function takes none of the names its enclosing functions use, read
    // there or not: no name shadows another, so each name means one
    // binding along any chain of scopes (siblings may share).
    let mut names: HashMap<SymbolId, String> = HashMap::new();
    let mut own: HashMap<ScopeId, Vec<String>> = HashMap::new();
    for (function, symbols) in &mut owned {
        symbols.sort_by_key(|&symbol| scoping.symbol_span(symbol).start);
        let mut reserved: BTreeSet<&str> = BTreeSet::new();
        let mut outer = scoping.scope_parent_id(*function);
        while let Some(scope) = outer {
            let enclosing = function_of(&scoping, scope);
            reserved.extend(own.get(&enclosing).into_iter().flatten().map(String::as_str));
            outer = scoping.scope_parent_id(enclosing);
        }
        let mut next = 0;
        let mut fresh = Vec::with_capacity(symbols.len());
        for _ in symbols.iter() {
            let name = loop {
                let candidate = letters(next);
                next += 1;
                if !reserved.contains(candidate.as_str()) && !taken.contains(candidate.as_str()) {
                    break candidate;
                }
            };
            fresh.push(name);
        }
        own.insert(*function, fresh.clone());
        for (&symbol, name) in symbols.iter().zip(fresh) {
            names.insert(symbol, name);
        }
    }

    let mut edits = Vec::new();
    for (&symbol, name) in &names {
        edits.push(analysis::Edit::replace(scoping.symbol_span(symbol), name.clone()));
        for reference in scoping.get_resolved_references(symbol) {
            edits.push(analysis::Edit::replace(nodes.kind(reference.node_id()).span(), name.clone()));
        }
    }
    if edits.is_empty() {
        return Ok((source.into(), 0));
    }
    let output = super::super::support::apply_checked(source, edits, "positional name")?;
    Ok((output, names.len()))
}

/// `$REUSED_20_0`, `$ARRAY_1_181`, …: everything the tracer and cleanup
/// named, except the thread functions and the shared slots.
fn generated(name: &str) -> bool {
    let Some(rest) = name.strip_prefix('$') else {
        return false;
    };
    let Some((prefix, _)) = rest.split_once('_') else {
        return false;
    };
    !matches!(prefix, "THREAD" | "KEY") && !prefix.is_empty() && prefix.bytes().all(|b| b.is_ascii_uppercase())
}

/// The nearest function scope at or above `scope`, or the program's.
fn function_of(scoping: &Scoping, mut scope: ScopeId) -> ScopeId {
    loop {
        if scoping.scope_flags(scope).is_function() {
            return scope;
        }
        match scoping.scope_parent_id(scope) {
            Some(parent) => scope = parent,
            None => return scope,
        }
    }
}

/// 0 → `$A`, 25 → `$Z`, 26 → `$AA`.
fn letters(mut number: usize) -> String {
    let mut out = Vec::new();
    loop {
        out.push(b'A' + (number % 26) as u8);
        if number < 26 {
            break;
        }
        number = number / 26 - 1;
    }
    out.push(b'$');
    out.reverse();
    String::from_utf8(out).unwrap()
}

/// Every `$`-name in the source, strings and comments included.
fn dollar_names(source: &str) -> BTreeSet<&str> {
    source
        .match_indices('$')
        .map(|(start, _)| {
            let tail = &source[start..];
            let end = tail[1..]
                .bytes()
                .position(|b| !(b.is_ascii_alphanumeric() || b == b'_' || b == b'$'))
                .map_or(tail.len(), |at| at + 1);
            &tail[..end]
        })
        .collect()
}

#[derive(Default)]
struct Dynamic {
    found: bool,
}

impl<'a> Visit<'a> for Dynamic {
    fn visit_with_statement(&mut self, statement: &WithStatement<'a>) {
        self.found = true;
        walk::walk_with_statement(self, statement);
    }

    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        if identifier.name == "eval" {
            self.found = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_each_functions_bindings_in_source_order() {
        let source = "function $THREAD_0_0() {\n\tlet $KEY_9 = 1;\n\tlet $REUSED_4_0 = 2;\n\tif ($KEY_9) {\n\t\tlet $JOIN_1_0 = $REUSED_4_0;\n\t}\n\tfunction $THREAD_1_5() {\n\t\tlet $ONCE_2_5 = 3;\n\t\treturn $ONCE_2_5;\n\t}\n}\n";
        let (code, count) = name_bindings_by_position(source).unwrap();
        assert_eq!(count, 3);
        assert_eq!(code, "function $THREAD_0_0() {\n\tlet $KEY_9 = 1;\n\tlet $A = 2;\n\tif ($KEY_9) {\n\t\tlet $B = $A;\n\t}\n\tfunction $THREAD_1_5() {\n\t\tlet $C = 3;\n\t\treturn $C;\n\t}\n}\n");
    }

    #[test]
    fn never_shadows_an_enclosing_name_but_siblings_share() {
        let source = "function $THREAD_0_0() {\n\tlet $REUSED_1_0 = 1;\n\tlet a = ($PARAM_0) => {\n\t\tlet $ONCE_2_5 = $PARAM_0;\n\t\treturn () => $ONCE_2_5;\n\t};\n\tlet b = ($PARAM_0) => $PARAM_0;\n\treturn [a, b, $REUSED_1_0];\n}\n";
        let (code, _) = name_bindings_by_position(source).unwrap();
        assert_eq!(code, "function $THREAD_0_0() {\n\tlet $A = 1;\n\tlet a = ($B) => {\n\t\tlet $C = $B;\n\t\treturn () => $C;\n\t};\n\tlet b = ($B) => $B;\n\treturn [a, b, $A];\n}\n");
    }

    #[test]
    fn skips_outer_names_a_nested_function_reads() {
        let source = "function $THREAD_0_0() {\n\tlet $REUSED_9_0 = 1;\n\tlet $VAR_8_0 = 2;\n\tfunction $THREAD_1_5() {\n\t\tlet $ONCE_2_5 = $VAR_8_0;\n\t\treturn () => $ONCE_2_5 + $REUSED_9_0;\n\t}\n}\n";
        let (code, _) = name_bindings_by_position(source).unwrap();
        assert_eq!(code, "function $THREAD_0_0() {\n\tlet $A = 1;\n\tlet $B = 2;\n\tfunction $THREAD_1_5() {\n\t\tlet $C = $B;\n\t\treturn () => $C + $A;\n\t}\n}\n");
    }

    #[test]
    fn letters_count_past_z() {
        assert_eq!(letters(0), "$A");
        assert_eq!(letters(25), "$Z");
        assert_eq!(letters(26), "$AA");
        assert_eq!(letters(27), "$AB");
        assert_eq!(letters(26 + 26 * 26), "$AAA");
    }
}
