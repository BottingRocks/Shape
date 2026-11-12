//! Flatten standalone statement-list blocks without changing lexical resolution.
//! Braces become spaces during the proof, keeping every identifier's byte span
//! stable. A batch must preserve reference-to-declaration identities, directives,
//! and semantic validity. Control-flow bodies and labeled blocks stay intact.

use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstKind, AstType};
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{Scoping, SemanticBuilder};
use oxc_span::{GetSpan, Span};

use crate::analysis;

#[derive(Default, PartialEq, Eq)]
struct Fingerprint {
    references: Vec<(Span, Option<Span>)>,
    directives: Vec<Span>,
    nodes: Vec<(AstType, Span, usize)>,
}

fn fingerprint(source: &str) -> Option<Fingerprint> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source).ok()?;
    let built = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&program);
    if !built.errors.is_empty() {
        return None;
    }
    struct Read<'s> {
        scoping: &'s Scoping,
        result: Fingerprint,
        depth: usize,
    }
    impl<'a> Visit<'a> for Read<'_> {
        fn enter_node(&mut self, kind: AstKind<'a>) {
            if !matches!(kind, AstKind::BlockStatement(_)) {
                self.result.nodes.push((kind.ty(), kind.span(), self.depth));
                self.depth += 1;
            }
        }
        fn leave_node(&mut self, kind: AstKind<'a>) {
            if !matches!(kind, AstKind::BlockStatement(_)) {
                self.depth -= 1;
            }
        }
        fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
            let declaration = analysis::identifier_symbol(id, self.scoping)
                .map(|symbol| self.scoping.symbol_span(symbol));
            self.result.references.push((id.span, declaration));
        }
        fn visit_directive(&mut self, directive: &Directive<'a>) {
            self.result.directives.push(directive.span);
        }
    }
    let mut read = Read {
        scoping: built.semantic.scoping(),
        result: Fingerprint::default(),
        depth: 0,
    };
    read.visit_program(&program);
    Some(read.result)
}

#[derive(Clone, Copy)]
struct Candidate {
    span: Span,
    lexical: bool,
}

#[derive(Default)]
struct Scan {
    blocks: Vec<Candidate>,
    dynamic: bool,
}

impl Scan {
    fn statements(&mut self, statements: &[Statement<'_>]) {
        for statement in statements {
            let Statement::BlockStatement(block) = statement else {
                continue;
            };
            // Block functions have Annex B/hoisting rules beyond ordinary
            // lexical reference resolution. Resource disposal must keep its
            // original end-of-scope point.
            if block.body.iter().any(|statement| match statement {
                Statement::FunctionDeclaration(_) => true,
                Statement::VariableDeclaration(declaration) => declaration.kind.is_using(),
                _ => false,
            }) {
                continue;
            }
            let lexical = block.body.iter().any(|statement| match statement {
                Statement::VariableDeclaration(declaration) => {
                    declaration.kind != VariableDeclarationKind::Var
                }
                Statement::ClassDeclaration(_) => true,
                _ => false,
            });
            self.blocks.push(Candidate {
                span: block.span,
                lexical,
            });
        }
    }
}

impl<'a> Visit<'a> for Scan {
    fn visit_program(&mut self, program: &Program<'a>) {
        self.statements(&program.body);
        walk::walk_program(self, program);
    }
    fn visit_function_body(&mut self, body: &FunctionBody<'a>) {
        self.statements(&body.statements);
        walk::walk_function_body(self, body);
    }
    fn visit_block_statement(&mut self, block: &BlockStatement<'a>) {
        self.statements(&block.body);
        walk::walk_block_statement(self, block);
    }
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        self.dynamic |= id.name == "eval";
    }
    fn visit_with_statement(&mut self, statement: &WithStatement<'a>) {
        self.dynamic = true;
        walk::walk_with_statement(self, statement);
    }
}

fn without_braces(source: &str, blocks: &[Candidate]) -> String {
    let mut bytes = source.as_bytes().to_vec();
    for block in blocks {
        bytes[block.span.start as usize] = b' ';
        bytes[block.span.end as usize - 1] = b' ';
    }
    // Only ASCII brace bytes were replaced; all offsets and UTF-8 survive.
    String::from_utf8(bytes).expect("replacing braces preserves UTF-8")
}

pub(crate) fn flatten_blocks(source: &str) -> Result<(String, usize), String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let mut scan = Scan::default();
    scan.visit_program(&program);
    if scan.dynamic || scan.blocks.is_empty() {
        return Ok((source.into(), 0));
    }
    let Some(original) = fingerprint(source) else {
        return Ok((source.into(), 0));
    };
    let code = without_braces(source, &scan.blocks);
    if fingerprint(&code).as_ref() == Some(&original) {
        return Ok((code, scan.blocks.len()));
    }
    // Keep lexical scopes if the combined lift collides or would capture an
    // outer read. Still flatten scope-free wrappers in one bounded retry;
    // never reparse once per block in large continuation chains.
    scan.blocks.retain(|block| !block.lexical);
    if !scan.blocks.is_empty() {
        let code = without_braces(source, &scan.blocks);
        if fingerprint(&code).as_ref() == Some(&original) {
            return Ok((code, scan.blocks.len()));
        }
    }
    Ok((source.into(), 0))
}

#[cfg(test)]
mod tests {
    use super::flatten_blocks;

    #[test]
    fn flattens_continuations_and_preserves_control_bodies() {
        let source = "function f(n) { for(let i=0;i<n;i++){ use(i); } { let gradient=make(); { while(n--){use(gradient);} { finish(gradient); } } } }";
        let (code, count) = flatten_blocks(source).unwrap();
        assert_eq!(count, 3);
        assert!(code.contains("for(let i=0;i<n;i++){ use(i); }"));
        assert!(code.contains("while(n--){use(gradient);}"));
        assert!(!code.contains("{ let gradient"));
    }

    #[test]
    fn keeps_shadowing_tdz_and_closure_resolution() {
        for source in [
            "function f(){let x=1;{let x=2;use(x);}return x;}",
            "function f(){use(typeof x);{let x=2;use(x);}}",
            "function f(){let read=()=>x;{let x=2;use(x);}return read;}",
            "function f(){{let x=1;use(x);}{let x=2;use(x);}}",
            "function f(){{let x=1;use(x);}if(flag){var x;}}",
        ] {
            let (code, count) = flatten_blocks(source).unwrap();
            assert_eq!(count, 0, "{source}");
            assert_eq!(code, source);
        }
    }

    #[test]
    fn preserves_directives_annex_b_and_resource_lifetimes() {
        for source in [
            "function f(){{'use strict';return this;}}",
            "function f(){{}'use strict';return this;}",
            "function f(){{function g(){return 1;} use(g);}return g;}",
            "function f(){{using resource=open();use(resource);}after();}",
        ] {
            assert_eq!(flatten_blocks(source).unwrap(), (source.into(), 0));
        }
    }

    #[test]
    fn retains_dynamic_scopes_and_labeled_jump_targets() {
        for source in [
            "function f(){{let x=1;eval('x');}}",
            "function f(obj){with(obj){{use(value);}}}",
            "function f(){done:{use();break done;}after();}",
        ] {
            assert_eq!(flatten_blocks(source).unwrap(), (source.into(), 0));
        }
    }

    #[test]
    fn keeps_comments_and_can_lift_destructuring_and_classes() {
        let source = "function f(){{/* α */let {x}=input;class Box{value=x;}use(new Box());}}";
        let (code, count) = flatten_blocks(source).unwrap();
        assert_eq!(count, 1);
        assert_eq!(code.len(), source.len());
        assert!(code.contains("/* α */"));
    }

    #[test]
    fn brace_removal_cannot_join_neighboring_expressions() {
        for source in [
            "function f(){ {factory()}(argument()); }",
            "function f(){factory()\n{(argument())} }",
        ] {
            assert_eq!(flatten_blocks(source).unwrap(), (source.into(), 0));
        }
    }

    #[test]
    fn lexical_conflict_does_not_block_scope_free_continuations() {
        let source = "function f(){let x=1;{let x=2;use(x);} {after();}}";
        let (code, count) = flatten_blocks(source).unwrap();
        assert_eq!(count, 1);
        assert!(code.contains("{let x=2;use(x);}"));
        assert!(!code.contains("{after();}"));
    }
}
