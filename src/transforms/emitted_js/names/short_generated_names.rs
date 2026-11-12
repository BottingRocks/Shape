//! Give linked program temporaries a short, deterministic function suffix.
//!
//! Names of the form `PREFIX<function-identity>_<serial>` (`$SSA_`,
//! `$CARRY_`, `$MERGE_`, `$ARRAY_`, `$OBJECT_`) become
//! `PREFIX<serial>_<number>`. Every binding/reference with the exact name is
//! replaced identically, so shadowing is preserved without semantic lookup.
//! The number comes from the linker's sorted function order. A replacement
//! that already occurs as an identifier anywhere makes the pass fail rather
//! than risk a collision. Strings, comments, and non-matching names are left
//! alone. This is a final byte/readability pass, not a statement cleanup.

use oxc_allocator::Allocator;
use oxc_ast::ast::{BindingIdentifier, IdentifierReference, Program};
use oxc_ast_visit::{walk, Visit};
use oxc_span::GetSpan;
use std::collections::HashSet;

use crate::analysis;

const PREFIXES: &[&str] = &["$SSA_", "$CARRY_", "$MERGE_", "$ARRAY_", "$OBJECT_"];

/// Rename generated values from `$SSA_<function identity>_<serial>` to
/// `$SSA_<serial>_<function number>`. Function numbers follow the linker's
/// sorted function order, so parallel trace scheduling cannot change them.
pub(crate) fn shorten_generated_names(
    source: &str,
    functions: &[(String, usize)],
) -> Result<String, String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let mut scan = Scan {
        functions,
        edits: Vec::new(),
        original_names: HashSet::new(),
    };
    scan.visit_program(&program);
    if scan.edits.is_empty() {
        return Ok(source.to_owned());
    }
    if scan
        .edits
        .iter()
        .any(|edit| scan.original_names.contains(edit.replacement.as_str()))
    {
        return Err("short generated name collides with an existing identifier".into());
    }
    let applied = analysis::apply_edits(source, scan.edits);
    if !applied.rejected.is_empty() {
        return Err("short generated-name edits overlap".into());
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output)
        .map_err(|error| format!("short generated-name output: {error}"))?;
    Ok(applied.output)
}

struct Scan<'a> {
    functions: &'a [(String, usize)],
    edits: Vec<analysis::Edit>,
    original_names: HashSet<String>,
}

impl Scan<'_> {
    fn rename(&mut self, name: &str, span: oxc_span::Span) {
        for prefix in PREFIXES {
            let Some(rest) = name.strip_prefix(prefix) else {
                continue;
            };
            for (function, number) in self.functions {
                let Some(serial) = rest
                    .strip_prefix(function.as_str())
                    .and_then(|rest| rest.strip_prefix('_'))
                else {
                    continue;
                };
                if !serial.is_empty() && serial.bytes().all(|byte| byte.is_ascii_digit()) {
                    self.edits.push(analysis::Edit::replace(
                        span,
                        format!("{prefix}{serial}_{number}"),
                    ));
                    return;
                }
            }
        }
    }
}

impl<'a> Visit<'a> for Scan<'_> {
    fn visit_program(&mut self, program: &Program<'a>) {
        walk::walk_program(self, program);
    }

    fn visit_binding_identifier(&mut self, identifier: &BindingIdentifier<'a>) {
        self.original_names.insert(identifier.name.to_string());
        self.rename(&identifier.name, identifier.span());
    }

    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        self.original_names.insert(identifier.name.to_string());
        self.rename(&identifier.name, identifier.span());
    }
}

#[cfg(test)]
mod tests {
    use super::shorten_generated_names;

    #[test]
    fn renames_only_identifier_tokens_with_stable_function_numbers() {
        let source = r#"function first() { let $SSA_2_46953_entry_9965 = 1; return $SSA_2_46953_entry_9965; }
function second() { let $SSA_7_26815_d3_9965 = 2; $CARRY_7_26815_d3_7 = $SSA_7_26815_d3_9965; return "$SSA_2_46953_entry_9965"; }"#;
        let functions = vec![("2_46953_entry".into(), 0), ("7_26815_d3".into(), 1)];
        let result = shorten_generated_names(source, &functions).unwrap();
        assert!(result.contains("let $SSA_9965_0 = 1; return $SSA_9965_0;"));
        assert!(result.contains("let $SSA_9965_1 = 2; $CARRY_7_1 = $SSA_9965_1;"));
        assert!(result.contains("\"$SSA_2_46953_entry_9965\""));
    }

    #[test]
    fn rejects_a_name_already_used_by_the_program() {
        let source = "let $SSA_2_46953_entry_9965 = 1; let $SSA_9965_0 = 2;";
        let functions = vec![("2_46953_entry".into(), 0)];
        let error = shorten_generated_names(source, &functions).unwrap_err();
        assert!(error.contains("collides"), "{error}");
    }
}
