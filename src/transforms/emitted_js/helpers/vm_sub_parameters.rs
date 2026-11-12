//! Give linked VM subroutine parameters short, stable positional names.
//!
//! Candidates are `function vm_sub_<digits>_<digits>` declarations whose
//! parameters are simple identifiers starting `vm_sub_`. Parameters become
//! `$PARAMA`, `$PARAMB`, ... and every resolved reference is renamed; the
//! function name is untouched. A textual collision check adds a numeric
//! suffix. A body containing `eval`, `with (`, or `with(` is skipped for this
//! pass. The guard is textual, not a syntax proof. This is a readability/byte
//! pass: calls, statements, and allocations are unchanged.

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::SymbolId;
use oxc_span::Span;
use oxc_syntax::scope::ScopeFlags;

use crate::analysis;

/// Rename `vm_sub_142618_3_m5` to `$PARAMA`, then `$PARAMB`, and so on in
/// declaration order. Positions describe the function signature, not a
/// guessed semantic role or the originating VM slot.
pub(crate) fn name_vm_sub_parameters(source: &str) -> Result<String, String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut functions = Functions::default();
    functions.visit_program(&program);
    let mut edits = Vec::new();
    for parameter in functions.parameters {
        let body =
            &source[parameter.function_span.start as usize..parameter.function_span.end as usize];
        // Dynamic scope may observe names that are only present in strings.
        if body.contains("eval") || body.contains("with (") || body.contains("with(") {
            continue;
        }
        let base = parameter_name(parameter.index);
        let mut name = base.clone();
        let mut suffix = 2;
        while identifier_occurs(body, &name) {
            name = format!("{base}_{suffix}");
            suffix += 1;
        }
        for span in analysis::symbol_all_spans(&scoping, &nodes, parameter.symbol) {
            edits.push(analysis::Edit::replace(span, name.clone()));
        }
    }
    if edits.is_empty() {
        return Ok(source.to_owned());
    }
    let applied = analysis::apply_edits(source, edits);
    if !applied.rejected.is_empty() {
        return Err("VM subroutine parameter edits overlap".into());
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output)
        .map_err(|error| format!("named VM subroutine output: {error}"))?;
    Ok(applied.output)
}

fn parameter_name(index: usize) -> String {
    let mut index = index + 1;
    let mut letters = String::new();
    while index > 0 {
        index -= 1;
        letters.insert(0, (b'A' + (index % 26) as u8) as char);
        index /= 26;
    }
    format!("$PARAM{letters}")
}

fn identifier_occurs(source: &str, name: &str) -> bool {
    source.match_indices(name).any(|(start, _)| {
        let identifier_char =
            |byte: u8| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$';
        let bytes = source.as_bytes();
        (start == 0 || !identifier_char(bytes[start - 1]))
            && (start + name.len() == bytes.len() || !identifier_char(bytes[start + name.len()]))
    })
}

struct Parameter {
    symbol: SymbolId,
    function_span: Span,
    index: usize,
}

#[derive(Default)]
struct Functions {
    parameters: Vec<Parameter>,
}

impl<'a> Visit<'a> for Functions {
    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        if let Some(id) = &function.id {
            let name = id.name.as_str();
            if name.strip_prefix("vm_sub_").is_some_and(|tail| {
                let mut parts = tail.split('_');
                parts.next().is_some_and(|part| {
                    !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit())
                }) && parts.next().is_some_and(|part| {
                    !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit())
                }) && parts.next().is_none()
            }) {
                for (index, parameter) in function.params.items.iter().enumerate() {
                    if let BindingPattern::BindingIdentifier(binding) = &parameter.pattern {
                        if binding.name.starts_with("vm_sub_") {
                            if let Some(symbol) = binding.symbol_id.get() {
                                self.parameters.push(Parameter {
                                    symbol,
                                    function_span: function.span,
                                    index,
                                });
                            }
                        }
                    }
                }
            }
        }
        walk::walk_function(self, function, flags);
    }
}

#[cfg(test)]
mod tests {
    use super::{name_vm_sub_parameters, parameter_name};

    #[test]
    fn names_parameters_by_signature_order_and_resolved_references() {
        let source = "function vm_sub_12_3(vm_sub_12_3_s2, vm_sub_12_3_s1) { let $SSA_1 = vm_sub_12_3_s2; return $SSA_1[vm_sub_12_3_s1]; }";
        let output = name_vm_sub_parameters(source).unwrap();
        assert!(
            output.contains("function vm_sub_12_3($PARAMA, $PARAMB)"),
            "{output}"
        );
        assert!(
            output.contains("let $SSA_1 = $PARAMA; return $SSA_1[$PARAMB]"),
            "{output}"
        );
    }

    #[test]
    fn skips_existing_local_name_without_changing_unrelated_functions() {
        let source = "function vm_sub_1_2(vm_sub_1_2_m5) { let $PARAMA = 2; return vm_sub_1_2_m5 + $PARAMA; } function other(vm_sub_1_2_m5) { return vm_sub_1_2_m5; }";
        let output = name_vm_sub_parameters(source).unwrap();
        assert!(
            output.contains("function vm_sub_1_2($PARAMA_2)"),
            "{output}"
        );
        assert!(output.contains("return $PARAMA_2 + $PARAMA"));
        assert!(output.contains("function other(vm_sub_1_2_m5)"));
    }

    #[test]
    fn keeps_dynamic_scope_untouched() {
        let source = "function vm_sub_1_2(vm_sub_1_2_m5) { return eval('vm_sub_1_2_m5'); }";
        assert_eq!(name_vm_sub_parameters(source).unwrap(), source);
    }

    #[test]
    fn extends_names_after_z() {
        assert_eq!(parameter_name(0), "$PARAMA");
        assert_eq!(parameter_name(25), "$PARAMZ");
        assert_eq!(parameter_name(26), "$PARAMAA");
    }
}
