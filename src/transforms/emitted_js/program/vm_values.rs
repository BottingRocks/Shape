//! Replace the VM values a traced program still names.
//!
//! - `vmNativeFunctions` is the kernel's array of builtins
//!   (`[ReferenceError, TypeError, Object, …]`): read once, it becomes that
//!   array literal. (Read more than once it stays a name, so every read sees
//!   one array.)
//! - `vmEmptyResultSentinel` is what the root thread hands back when it ends
//!   without a value; the program discards the root's result, so the root's
//!   `return vmEmptyResultSentinel;` is a plain `return;`, and goes when it
//!   ends the body.
//! - `vmEntry` is the VM's own interpreter function, read only to
//!   fingerprint its source: [`super::vm_fingerprint`] folds that into the
//!   numbers it gives on the raw script.

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_span::{GetSpan, Span};

use crate::analysis;
use super::super::support::whole_line;

/// `raw` is the raw script the program was traced from.
pub(crate) fn replace_vm_values(source: &str, native_functions: &[String], raw: &str) -> Result<(String, usize), String> {
    if !source.contains("vmNativeFunctions") && !source.contains("vmEmptyResultSentinel") && !source.contains("vmEntry") {
        return Ok((source.into(), 0));
    }
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut edits = Vec::new();

    let natives: Vec<Span> = scoping
        .root_unresolved_references()
        .iter()
        .filter(|(name, _)| name.as_str() == "vmNativeFunctions")
        .flat_map(|(_, references)| references.iter().map(|&reference| nodes.get_node(scoping.get_reference(reference).node_id()).span()))
        .collect();
    if let ([read], false) = (natives.as_slice(), native_functions.is_empty()) {
        edits.push(analysis::Edit::replace(*read, format!("[{}]", native_functions.join(", "))));
    }

    if let Some(entry) = super::super::vm_fingerprint::fold_entry_fingerprint(source, &scoping, &nodes, raw) {
        edits.extend(entry);
    }

    // The root thread: the one the program's last statement calls.
    let root = program.body.last().and_then(|statement| {
        let Statement::ExpressionStatement(statement) = statement else { return None };
        let Expression::CallExpression(call) = &statement.expression else { return None };
        let Expression::Identifier(callee) = &call.callee else { return None };
        program.body.iter().find_map(|statement| match statement {
            Statement::FunctionDeclaration(function) if function.id.as_ref().is_some_and(|id| id.name == callee.name) => Some(function),
            _ => None,
        })
    });
    if let Some(body) = root.and_then(|function| function.body.as_ref()) {
        let mut returns = Returns { found: Vec::new(), depth: 0 };
        oxc_ast_visit::Visit::visit_function_body(&mut returns, body);
        let last = body.statements.last().map(GetSpan::span);
        for span in returns.found {
            if Some(span) == last {
                edits.push(analysis::Edit::delete(whole_line(source, span)));
            } else {
                edits.push(analysis::Edit::replace(span, "return;"));
            }
        }
    }

    let count = edits.len();
    if count == 0 {
        return Ok((source.into(), 0));
    }
    let output = super::super::support::apply_checked(source, edits, "vm value")?;
    Ok((output, count))
}

/// `return vmEmptyResultSentinel;` in the function itself, not nested ones.
struct Returns {
    found: Vec<Span>,
    depth: usize,
}

impl<'a> oxc_ast_visit::Visit<'a> for Returns {
    fn visit_function(&mut self, function: &Function<'a>, flags: oxc_syntax::scope::ScopeFlags) {
        self.depth += 1;
        oxc_ast_visit::walk::walk_function(self, function, flags);
        self.depth -= 1;
    }
    fn visit_arrow_function_expression(&mut self, arrow: &ArrowFunctionExpression<'a>) {
        self.depth += 1;
        oxc_ast_visit::walk::walk_arrow_function_expression(self, arrow);
        self.depth -= 1;
    }
    fn visit_return_statement(&mut self, statement: &ReturnStatement<'a>) {
        if self.depth == 0 && matches!(&statement.argument, Some(Expression::Identifier(value)) if value.name == "vmEmptyResultSentinel") {
            self.found.push(statement.span);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inlines_the_native_catalogue_and_ends_the_root_plainly() {
        let source = "function $THREAD_0_0() {\n\tlet $KEY_1 = vmNativeFunctions;\n\tif ($KEY_1) {\n\t\treturn vmEmptyResultSentinel;\n\t}\n\tf(() => vmEmptyResultSentinel);\n\treturn vmEmptyResultSentinel;\n}\n\n$THREAD_0_0();\n";
        let natives = vec!["ReferenceError".to_string(), "Function.prototype.bind".to_string()];
        let (code, count) = replace_vm_values(source, &natives, "").unwrap();
        assert_eq!(count, 3);
        assert_eq!(code, "function $THREAD_0_0() {\n\tlet $KEY_1 = [ReferenceError, Function.prototype.bind];\n\tif ($KEY_1) {\n\t\treturn;\n\t}\n\tf(() => vmEmptyResultSentinel);\n}\n\n$THREAD_0_0();\n");
    }
}
