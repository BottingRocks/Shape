//! `L: for (;;) { …; continue outer; }` → `{ …; continue outer; }`: a loop
//! nothing jumps back to and whose body cannot reach its end runs its body
//! once. The tracer emits such a loop where a walk entered a position it
//! had to treat as a loop head but every path from it left through an
//! enclosing loop's jump; the wrapper then hides that enclosing loop's
//! shape from [`reconstruct_loops`](super::reconstruct_loops).

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_span::{GetSpan, Span};

use super::{block, cannot_fall_through, inside, Jumps};
use crate::analysis;

/// Unwraps every single-pass `for (;;)` loop; returns the code and how many
/// went.
pub(crate) fn unwrap_single_pass_loops(source: &str) -> Result<(String, usize), String> {
    let mut code = source.to_string();
    let mut unwrapped = 0;
    for _ in 0..64 {
        if !code.contains("for (;;)") {
            break;
        }
        let allocator = Allocator::default();
        let program = analysis::parse_program(&allocator, &code)?;
        let mut scan = Scan { source: &code, found: Vec::new() };
        scan.visit_program(&program);
        // Outermost first; a nested one goes next round.
        let mut edits: Vec<analysis::Edit> = Vec::new();
        let mut taken: Vec<Span> = Vec::new();
        scan.found.sort_by_key(|(span, _)| (span.start, std::cmp::Reverse(span.end)));
        for (span, body) in scan.found {
            if taken.iter().any(|outer| inside(*outer, span)) {
                continue;
            }
            taken.push(span);
            edits.push(analysis::Edit::replace(span, code[body.start as usize..body.end as usize].to_string()));
        }
        if edits.is_empty() {
            break;
        }
        unwrapped += edits.len();
        let applied = analysis::apply_edits(&code, edits);
        if !applied.rejected.is_empty() {
            return Err("single-pass loop edits overlap".into());
        }
        code = applied.output;
    }
    if unwrapped > 0 {
        let check = Allocator::default();
        analysis::check_program(&check, &code).map_err(|error| format!("single-pass loop output: {error}"))?;
    }
    Ok((code, unwrapped))
}

struct Scan<'s> {
    source: &'s str,
    /// Each loop statement (with its label) and its body block.
    found: Vec<(Span, Span)>,
}

impl Scan<'_> {
    /// The body block of a `for (;;)` that runs its body once, given the
    /// loop's label (`""` for none).
    fn single_pass(loop_: &ForStatement<'_>, label: &str) -> Option<Span> {
        if loop_.init.is_some() || loop_.test.is_some() || loop_.update.is_some() {
            return None;
        }
        let body = block(&loop_.body)?;
        if !body.last().is_some_and(cannot_fall_through) {
            return None;
        }
        let mut jumps = Jumps::new(label);
        jumps.statements(body);
        (jumps.breaks == 0 && jumps.continues == 0).then(|| loop_.body.span())
    }
}

impl<'a> Visit<'a> for Scan<'_> {
    fn visit_with_statement(&mut self, _: &WithStatement<'a>) {
        // Leave `with` bodies alone (their jumps are ordinary, but nothing
        // here needs them).
    }

    fn visit_labeled_statement(&mut self, labeled: &LabeledStatement<'a>) {
        if let Statement::ForStatement(loop_) = &labeled.body {
            if let Some(body) = Self::single_pass(loop_, labeled.label.name.as_str()) {
                self.found.push((labeled.span, body));
            }
            // Visit the loop's parts, not the loop again as unlabeled.
            walk::walk_for_statement(self, loop_);
            return;
        }
        walk::walk_labeled_statement(self, labeled);
    }

    fn visit_for_statement(&mut self, loop_: &ForStatement<'a>) {
        if let Some(body) = Self::single_pass(loop_, "") {
            self.found.push((loop_.span, body));
        }
        walk::walk_for_statement(self, loop_);
    }
}

#[cfg(test)]
mod tests {
    use super::unwrap_single_pass_loops;

    #[test]
    fn unwraps_loops_that_leave_through_an_outer_jump() {
        let source = "function f() {\nvm_loop_1: for (;;) {\n  if (a()) {\n    vm_loop_2: for (;;) {\n      b();\n      vm_loop_3: for (;;) {\n        c();\n        continue vm_loop_1;\n      }\n    }\n  } else {\n    return 1;\n  }\n}\n}\n";
        let (code, count) = unwrap_single_pass_loops(source).unwrap();
        assert_eq!(count, 2);
        assert!(!code.contains("vm_loop_2") && !code.contains("vm_loop_3"), "{code}");
        assert!(code.contains("vm_loop_1: for (;;)"), "{code}");
        assert!(code.contains("continue vm_loop_1;"), "{code}");
    }

    #[test]
    fn keeps_loops_that_iterate_or_fall_through() {
        for source in [
            // Its own continue.
            "for (;;) { if (a()) { continue; } return; }",
            "L: for (;;) { if (a()) { continue L; } return; }",
            // A break leaves it: the code after the loop runs.
            "for (;;) { if (a()) { break; } return; }\nb();",
            // The body can reach its end and run again.
            "for (;;) { if (a()) { return; } }",
            // A header.
            "for (; a(); ) { return; }",
        ] {
            let source = format!("function f() {{ {source} }}");
            let (code, count) = unwrap_single_pass_loops(&source).unwrap();
            assert_eq!(count, 0, "{source} → {code}");
        }
    }

    #[test]
    fn a_nested_loops_own_jumps_do_not_count() {
        let source = "function f() { for (;;) { while (a()) { if (b()) break; continue; } return 1; } }";
        let (code, count) = unwrap_single_pass_loops(source).unwrap();
        assert_eq!(count, 1, "{code}");
        assert!(code.contains("{ { while"), "{code}");
    }
}
