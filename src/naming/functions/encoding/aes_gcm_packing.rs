//! Bounded, literal/API-preserving AST proofs for packed AES-GCM payloads.
//! The specifications below describe complete algorithms, including owned arrow
//! bodies. Scoped symbols are matched bijectively; source spellings, labels,
//! function ordinals and build constants never participate.
use super::*;
use oxc_semantic::{Scoping, SymbolId};
use std::sync::LazyLock;

#[path = "aes_key_preparation.rs"]
mod key_preparation;

#[derive(Clone, Debug, PartialEq)]
enum Tree {
    Node(String, Vec<Tree>),
    Local(SymbolId),
    Global(String),
}
fn node(tag: impl Into<String>, children: Vec<Tree>) -> Tree {
    Tree::Node(tag.into(), children)
}
fn leaf(tag: impl Into<String>) -> Tree {
    node(tag, vec![])
}

struct Syntax<'a> {
    scopes: &'a Scoping,
    remaining: usize,
}
impl Syntax<'_> {
    fn tick(&mut self) -> Option<()> {
        self.remaining = self.remaining.checked_sub(1)?;
        Some(())
    }
    fn identifier(&self, i: &IdentifierReference<'_>) -> Tree {
        crate::analysis::identifier_symbol(i, self.scopes)
            .map(Tree::Local)
            .unwrap_or_else(|| Tree::Global(i.name.to_string()))
    }
    fn binding(&self, p: &BindingPattern<'_>) -> Option<Tree> {
        let BindingPattern::BindingIdentifier(i) = p else {
            return None;
        };
        let s = i.symbol_id.get()?;
        self.scopes
            .symbol_redeclarations(s)
            .is_empty()
            .then_some(Tree::Local(s))
    }
    fn function(&mut self, f: &Function<'_>) -> Option<Tree> {
        if f.r#async || f.generator {
            return None;
        }
        let body = f.body.as_ref()?;
        if !body.directives.is_empty() {
            return None;
        }
        Some(node(
            "function",
            vec![self.params(&f.params)?, self.statements(&body.statements)?],
        ))
    }
    fn params(&self, p: &FormalParameters<'_>) -> Option<Tree> {
        if p.rest.is_some() {
            return None;
        }
        Some(node(
            "params",
            p.items
                .iter()
                .map(|p| {
                    if p.initializer.is_some() {
                        return None;
                    }
                    self.binding(&p.pattern)
                })
                .collect::<Option<_>>()?,
        ))
    }
    fn statements(&mut self, statements: &[Statement<'_>]) -> Option<Tree> {
        let mut out = Vec::new();
        for s in statements {
            let next = self.statement(s)?;
            // Adjacent `let x; x = value` has no intervening use. Normalize this
            // one supported recovered form; never move assignments across code.
            if let Tree::Node(tag, assignments) = &next {
                if tag == "expression" && assignments.len() == 1 {
                    if let Tree::Node(op, sides) = &assignments[0] {
                        if op == "assign:=" && sides.len() == 2 {
                            if let Some(Tree::Node(decl, entries)) = out.last_mut() {
                                if decl == "variable" && entries.len() == 1 {
                                    if let Tree::Node(entry, fields) = &mut entries[0] {
                                        if entry == "binding"
                                            && fields.len() == 1
                                            && fields[0] == sides[0]
                                        {
                                            fields.push(sides[1].clone());
                                            continue;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            out.push(next);
        }
        Some(node("statements", out))
    }
    fn statement(&mut self, s: &Statement<'_>) -> Option<Tree> {
        self.tick()?;
        Some(match s {
            Statement::VariableDeclaration(d)
                if matches!(
                    d.kind,
                    VariableDeclarationKind::Var
                        | VariableDeclarationKind::Let
                        | VariableDeclarationKind::Const
                ) =>
            {
                node(
                    "variable",
                    d.declarations
                        .iter()
                        .map(|v| {
                            let mut children = vec![self.binding(&v.id)?];
                            if let Some(e) = &v.init {
                                children.push(self.expression(e)?);
                            }
                            Some(node("binding", children))
                        })
                        .collect::<Option<_>>()?,
                )
            }
            Statement::ExpressionStatement(s) => {
                node("expression", vec![self.expression(&s.expression)?])
            }
            Statement::ReturnStatement(s) => node(
                "return",
                s.argument
                    .iter()
                    .map(|e| self.expression(e))
                    .collect::<Option<_>>()?,
            ),
            Statement::BlockStatement(s) => node("block", vec![self.statements(&s.body)?]),
            Statement::IfStatement(s) => {
                let mut parts = vec![self.expression(&s.test)?, self.statement(&s.consequent)?];
                if let Some(other) = &s.alternate {
                    parts.push(self.statement(other)?);
                }
                node("if", parts)
            }
            _ => return None,
        })
    }
    fn expression(&mut self, e: &Expression<'_>) -> Option<Tree> {
        self.tick()?;
        Some(match e {
            Expression::Identifier(i) => self.identifier(i),
            Expression::ParenthesizedExpression(p) => return self.expression(&p.expression),
            Expression::StringLiteral(s) => leaf(format!("string:{}", s.value)),
            Expression::NumericLiteral(n) => leaf(format!("number:{}", n.value)),
            Expression::BooleanLiteral(b) => leaf(format!("bool:{}", b.value)),
            Expression::NullLiteral(_) => leaf("null"),
            Expression::StaticMemberExpression(m) => node(
                format!("member:{}:{}", m.optional, m.property.name),
                vec![self.expression(&m.object)?],
            ),
            Expression::ComputedMemberExpression(m) => node(
                format!("index:{}", m.optional),
                vec![self.expression(&m.object)?, self.expression(&m.expression)?],
            ),
            Expression::ChainExpression(c) => {
                let inner = match &c.expression {
                    ChainElement::CallExpression(c) => self.call(c)?,
                    ChainElement::StaticMemberExpression(m) => node(
                        format!("member:{}:{}", m.optional, m.property.name),
                        vec![self.expression(&m.object)?],
                    ),
                    ChainElement::ComputedMemberExpression(m) => node(
                        format!("index:{}", m.optional),
                        vec![self.expression(&m.object)?, self.expression(&m.expression)?],
                    ),
                    _ => return None,
                };
                node("chain", vec![inner])
            }
            Expression::CallExpression(c) => self.call(c)?,
            Expression::NewExpression(n) => {
                let mut parts = vec![self.expression(&n.callee)?];
                parts.extend(
                    n.arguments
                        .iter()
                        .map(|a| self.argument(a))
                        .collect::<Option<Vec<_>>>()?,
                );
                node("new", parts)
            }
            Expression::BinaryExpression(b) => node(
                format!("binary:{}", b.operator.as_str()),
                vec![self.expression(&b.left)?, self.expression(&b.right)?],
            ),
            Expression::LogicalExpression(b) => node(
                format!("logical:{}", b.operator.as_str()),
                vec![self.expression(&b.left)?, self.expression(&b.right)?],
            ),
            Expression::UnaryExpression(u) => node(
                format!("unary:{}", u.operator.as_str()),
                vec![self.expression(&u.argument)?],
            ),
            Expression::ConditionalExpression(c) => node(
                "conditional",
                vec![
                    self.expression(&c.test)?,
                    self.expression(&c.consequent)?,
                    self.expression(&c.alternate)?,
                ],
            ),
            Expression::ArrayExpression(a) => node(
                "array",
                a.elements
                    .iter()
                    .map(|e| self.expression(e.as_expression()?))
                    .collect::<Option<_>>()?,
            ),
            Expression::ObjectExpression(o) => {
                let mut properties = BTreeMap::new();
                for p in &o.properties {
                    let ObjectPropertyKind::ObjectProperty(p) = p else {
                        return None;
                    };
                    if p.computed || p.method || p.kind != PropertyKind::Init {
                        return None;
                    }
                    let key = property_key_name(&p.key)?;
                    if properties.insert(key, self.expression(&p.value)?).is_some() {
                        return None;
                    }
                }
                node(
                    "object",
                    properties
                        .into_iter()
                        .map(|(k, v)| node(k, vec![v]))
                        .collect(),
                )
            }
            Expression::ArrowFunctionExpression(a) => {
                if a.r#async || !a.body.directives.is_empty() {
                    return None;
                }
                node(
                    format!("arrow:{}", a.expression),
                    vec![
                        self.params(&a.params)?,
                        self.statements(&a.body.statements)?,
                    ],
                )
            }
            Expression::AssignmentExpression(a) => {
                let target = match &a.left {
                    AssignmentTarget::AssignmentTargetIdentifier(i) => self.identifier(i),
                    AssignmentTarget::StaticMemberExpression(m) => node(
                        format!("member:{}:{}", m.optional, m.property.name),
                        vec![self.expression(&m.object)?],
                    ),
                    AssignmentTarget::ComputedMemberExpression(m) => node(
                        format!("index:{}", m.optional),
                        vec![self.expression(&m.object)?, self.expression(&m.expression)?],
                    ),
                    _ => return None,
                };
                node(
                    format!("assign:{}", a.operator.as_str()),
                    vec![target, self.expression(&a.right)?],
                )
            }
            _ => return None,
        })
    }
    fn argument(&mut self, a: &Argument<'_>) -> Option<Tree> {
        self.expression(a.as_expression()?)
    }
    fn call(&mut self, c: &CallExpression<'_>) -> Option<Tree> {
        let mut children = vec![self.expression(&c.callee)?];
        children.extend(
            c.arguments
                .iter()
                .map(|a| self.argument(a))
                .collect::<Option<Vec<_>>>()?,
        );
        Some(node(format!("call:{}", c.optional), children))
    }
}

#[derive(Default)]
struct Match {
    locals: BTreeMap<SymbolId, SymbolId>,
    captures: BTreeMap<String, SymbolId>,
}
impl Match {
    fn tree(
        &mut self,
        expected: &Tree,
        actual: &Tree,
        model: &Model<'_, '_>,
        site: FunctionSite,
    ) -> bool {
        match (expected, actual) {
            (Tree::Local(a), Tree::Local(b)) => {
                if !site.span.contains_inclusive(model.scoping.symbol_span(*b))
                    || !model.scoping.symbol_redeclarations(*b).is_empty()
                {
                    return false;
                }
                if let Some(old) = self.locals.get(a) {
                    return old == b;
                }
                if self.locals.values().any(|s| s == b) {
                    return false;
                }
                self.locals.insert(*a, *b);
                true
            }
            (Tree::Global(a), Tree::Local(b)) if a.starts_with("CAP_") => {
                if site.span.contains_inclusive(model.scoping.symbol_span(*b)) {
                    return false;
                }
                if let Some(old) = self.captures.get(a) {
                    return old == b;
                }
                if self.captures.values().any(|s| s == b) {
                    return false;
                }
                self.captures.insert(a.clone(), *b);
                true
            }
            // Message text is immaterial to the null-result diagnostic. This is
            // the only literal wildcard; algorithm parameters remain exact.
            (Tree::Node(a, ac), Tree::Node(b, bc)) if a == "string:DIAGNOSTIC" => {
                b.starts_with("string:") && ac.is_empty() && bc.is_empty()
            }
            (Tree::Node(a, ac), Tree::Node(b, bc)) => {
                a == b
                    && ac.len() == bc.len()
                    && ac.iter().zip(bc).all(|(a, b)| self.tree(a, b, model, site))
            }
            (Tree::Global(a), Tree::Global(b)) => a == b,
            _ => false,
        }
    }
}

fn compile(source: &str, expression: bool) -> Tree {
    let allocator = Allocator::default();
    let program = crate::analysis::parse_program(&allocator, source)
        .expect("packing proof specification parses");
    let (scopes, _) = crate::analysis::analyze(&program);
    let mut syntax = Syntax {
        scopes: &scopes,
        remaining: 4096,
    };
    let result = if expression {
        let Statement::ExpressionStatement(s) = &program.body[0] else {
            unreachable!()
        };
        syntax.expression(&s.expression)
    } else {
        let Statement::FunctionDeclaration(f) = &program.body[0] else {
            unreachable!()
        };
        syntax.function(f)
    };
    result.expect("packing proof specification uses supported grammar")
}

// Generic complete string-to-packed-blob algorithm. These are semantic AST
// specifications, not samples or hashes of recovered programs.
const ENCRYPT: &str = r#"function encrypt(input){
let encoded=new TextEncoder().encode(input);let pending;
if(CAP_SUBTLE!=null){pending=CAP_SUBTLE.generateKey({name:"AES-GCM",length:128},true,["encrypt","decrypt"]).then(key=>key);}else{pending=null;}
if(pending!=null){return pending.then(generated=>{
let key=generated;let iv=CAP_CONTEXT.crypto?.getRandomValues(new Uint8Array(12));
return CAP_SUBTLE.encrypt({name:"AES-GCM",iv:iv},key,encoded).then(encrypted=>{
let bytes=encrypted;return CAP_SUBTLE.exportKey("raw",key).then(raw=>{
let data=bytes;let nonce=iv;let packed=new Uint8Array(data.byteLength+raw.byteLength+nonce.byteLength);
packed.set(new Uint8Array(raw),0);packed.set(new Uint8Array(nonce),raw.byteLength);
packed.set(new Uint8Array(data),raw.byteLength+nonce.byteLength);return packed.buffer;
});});});}else{return null;}}
"#;
const DECRYPT: &str = r#"function decrypt(input){
let buffer=new Uint8Array(input.split(",").map(item=>parseInt(item,10))).buffer;
let key=buffer.slice(0,16);let iv=buffer.slice(16,28);let data=buffer.slice(28,buffer.byteLength);
key={extractedKey:key,extractedIv:void 0,extractedData:void 0};key.extractedIv=iv;key.extractedData=data;
let encrypted=key.extractedData;let nonce=key.extractedIv;
return CAP_SUBTLE.importKey("raw",key.extractedKey,"AES-GCM",true,["encrypt","decrypt"]).then(imported=>{
let algorithm={name:"AES-GCM"};algorithm.iv=nonce;
return CAP_SUBTLE.decrypt(algorithm,imported,encrypted).then(result=>result);
}).then(result=>{let value=JSON.parse(new TextDecoder().decode(result)||"");
if(value==null){CAP_DIAGNOSTIC({message:"DIAGNOSTIC"});return void 0;}else{return value;}});}
"#;

// Keep each accepted whole-body proof separate from its explicit-IV extension.
static ENCRYPT_TREES: LazyLock<Vec<(Tree, Tree)>> = LazyLock::new(|| {
    // An own data descriptor with every default-object attribute true has the
    // same AES key-length value. Match the complete construction and receiver
    // alias, rather than treating arbitrary defineProperty calls as literals.
    let descriptor = r#"let service=CAP_SUBTLE;let algorithm={name:"AES-GCM"};
Object.defineProperty(algorithm,"length",{writable:true,configurable:true,enumerable:true,value:128});
pending=service.generateKey(algorithm,true,["encrypt","decrypt"]).then(key=>key);"#;
    let direct = r#"pending=CAP_SUBTLE.generateKey({name:"AES-GCM",length:128},true,["encrypt","decrypt"]).then(key=>key);"#;
    let expanded = ENCRYPT.replace(direct, descriptor);
    let without_service = expanded.replace("let service=CAP_SUBTLE;", "").replace(
        "pending=service.generateKey",
        "pending=CAP_SUBTLE.generateKey",
    );
    // The recovered variant frames the payload from the IV's backing
    // ArrayBuffer (`iv.buffer`) instead of reusing the IV view. The template
    // keeps the same `iv` local in the encrypt `iv:` option, so this byte
    // framing can only prove when it really derives from that exact IV.
    let buffered = |source: &str| source.replace("let nonce=iv;", "let nonce=iv.buffer;");
    let originals = vec![
        ENCRYPT.to_string(), expanded.clone(), without_service.clone(),
        buffered(ENCRYPT), buffered(&expanded), buffered(&without_service),
    ];
    let mut trees = Vec::new();
    for source in originals {
        let legacy = compile(&source, false);
        // Preserve the exact null guard, context receiver and 12-byte IV; do
        // not absorb arbitrary statements while lowering optional chaining.
        let explicit_iv = compile(&source.replace("let iv=CAP_CONTEXT.crypto?.getRandomValues(new Uint8Array(12));",
            "let iv;if(CAP_CONTEXT.crypto==null){iv=void 0;}else{iv=CAP_CONTEXT.crypto.getRandomValues(new Uint8Array(12));}"), false);
        trees.push((legacy, explicit_iv));
    }
    trees

});
static DECRYPT_TREES: LazyLock<Vec<Tree>> = LazyLock::new(|| {
    // Both direct slices and the recovered receiver-preserving `.call` form
    // prove the same slice boundaries. Each complete alternative is checked.
    let mut sources = vec![DECRYPT.to_string()];
    let call = DECRYPT.replace("let key=buffer.slice(0,16);let iv=buffer.slice(16,28);",
        "let pair=[buffer,buffer.slice];let key=pair[1].call(pair[0],0,16);let other=[buffer,buffer.slice];let iv=other[1].call(other[0],16,28);");
    sources.push(call);
    let mut variants = Vec::new();
    for source in sources {
        variants.push(source.clone());
        // A separate record with the same proven fields and later assignments.
        variants.push(source.replace("key={extractedKey:key,extractedIv:void 0,extractedData:void 0};key.extractedIv=iv;key.extractedData=data;",
            "let record={extractedKey:void 0,extractedIv:void 0,extractedData:void 0};record.extractedKey=key;record.extractedIv=iv;record.extractedData=data;")
            .replace("key.extracted", "record.extracted"));
    }
    let mut trees = Vec::new();
    for source in variants {
        trees.push(compile(&source, false));
        // Decode, fallback and JSON parsing remain contiguous and ordered.
        trees.push(compile(&source.replace("let value=JSON.parse(new TextDecoder().decode(result)||\"\");",
            "let value=new TextDecoder().decode(result);if(!value){value=\"\";}value=JSON.parse(value);"), false));
    }
    trees
});
static SUBTLE_ORIGIN: LazyLock<Tree> =
    LazyLock::new(|| compile("CAP_CONTEXT.crypto?.subtle;", true));

/// Only plain variables with one initializer or one assignment can establish a
/// captured service identity. Parameters/catches/destructuring/loop bindings and
/// redeclarations carry additional runtime origins and cannot supply this proof.
fn capture_value<'a>(model: &Model<'_, 'a>, symbol: SymbolId) -> Option<&'a Expression<'a>> {
    if !model.scoping.symbol_redeclarations(symbol).is_empty() {
        return None;
    }
    let declaration = model.scoping.symbol_declaration(symbol);
    let AstKind::VariableDeclarator(v) = model.nodes.kind(declaration) else {
        return None;
    };
    if !matches!(&v.id,BindingPattern::BindingIdentifier(i) if i.symbol_id.get()==Some(symbol))
        || !matches!(
            model.nodes.parent_kind(declaration),
            AstKind::VariableDeclaration(_)
        )
        || matches!(
            model.nodes.parent_kind(model.nodes.parent_id(declaration)),
            AstKind::ForInStatement(_) | AstKind::ForOfStatement(_)
        )
        || !(model.binding_writes(symbol) == 0 && v.init.is_some()
            || model.binding_writes(symbol) == 1 && v.init.is_none())
    {
        return None;
    }
    model.symbol_value(symbol)
}

fn prove(model: &Model<'_, '_>, site: FunctionSite, expected: &Tree, actual: &Tree) -> bool {
    let mut matched = Match::default();
    if !matched.tree(expected, actual, model, site) {
        return false;
    }
    let Some(subtle) = matched.captures.get("CAP_SUBTLE").copied() else {
        return false;
    };
    let Some(origin) = capture_value(model, subtle) else {
        return false;
    };
    let Some(origin) = (Syntax {
        scopes: model.scoping,
        remaining: 64,
    })
    .expression(origin) else {
        return false;
    };
    if !matched.tree(&SUBTLE_ORIGIN, &origin, model, site) {
        return false;
    }
    let Some(context) = matched.captures.get("CAP_CONTEXT").copied() else {
        return false;
    };
    if capture_value(model, context).is_none() {
        return false;
    }
    if let Some(diagnostic) = matched.captures.get("CAP_DIAGNOSTIC") {
        // It is only a diagnostic side effect, but the callable must still have
        // an unambiguous stable source origin; labels never establish that.
        let decl = model.scoping.symbol_declaration(*diagnostic);
        if let AstKind::Function(_) = model.nodes.kind(decl) {
            if model.binding_writes(*diagnostic) != 0
                || !model.scoping.symbol_redeclarations(*diagnostic).is_empty()
            {
                return false;
            }
        } else {
            let Some(value) = capture_value(model, *diagnostic) else {
                return false;
            };
            if stable_function(model, value, 0).is_none() {
                return false;
            }
        }
    }
    true
}

pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    f: &Facts<'_>,
    _proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
    let preparation = key_preparation::evaluate(model, site, f);
    // Cheap owned-facts gate before compiling this one bounded body. Nested
    // callbacks are then inspected as part of the complete algorithm proof.
    if !f.members.contains("then")
        || !(f.members.contains("generateKey") || f.members.contains("importKey"))
    {
        return vec![];
    }
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return vec![];
    };
    let Some(actual) = (Syntax {
        scopes: model.scoping,
        remaining: 4096,
    })
    .function(function) else {
        return preparation;
    };
    // Preserve the original packing priority and canonical metadata. Only a
    // previously unsupported packing body may reach the explicit-IV extension;
    // a closed legacy preparer proof still owns its original claim even when
    // its opaque continuation now also has a complete encryption proof.
    let legacy_encrypt = ENCRYPT_TREES
        .iter()
        .any(|(legacy, _)| prove(model, site, legacy, &actual));
    if !legacy_encrypt && !preparation.is_empty() {
        return preparation;
    }
    let mut claims = Vec::new();
    if legacy_encrypt || ENCRYPT_TREES
        .iter()
        .any(|(_, explicit_iv)| prove(model, site, explicit_iv, &actual))
    {
        claims.push(proposal("encryptStringAesGcm","shared_packing_algorithm","complete TextEncoder / AES-GCM-128 / random 12-byte IV / raw-key export / key-IV-ciphertext packing proof, including every owned callback and stable captured crypto identity",f));
    }
    if DECRYPT_TREES
        .iter()
        .any(|expected| prove(model, site, expected, &actual))
    {
        claims.push(proposal("decryptPackedAesGcmBlob","shared_packing_algorithm","complete decimal-byte parse, 16-byte key and 12-byte IV slices, raw AES-GCM import/decrypt, TextDecoder/JSON parse and exact null-result finalization; every callback and capture identity proved",f));
    }
    if claims.is_empty() {
        preparation
    } else {
        claims
    }
}

/// Reuse the bounded scoped syntax for complete algorithms with no external
/// captures. Every parameter, declaration, nested callback and effect is still
/// matched bijectively; unbound globals must retain exact identity.
pub(in crate::naming) fn complete_uncaptured(
    model: &Model<'_, '_>, site: FunctionSite, specification: &str,
) -> bool {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else { return false; };
    let Some(actual) = (Syntax { scopes: model.scoping, remaining: 4096 }).function(function) else { return false; };
    let mut matched = Match::default();
    matched.tree(&compile(specification, false), &actual, model, site) && matched.captures.is_empty()
}
