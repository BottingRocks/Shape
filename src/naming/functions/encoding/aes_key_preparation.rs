//! Parent-owned encoding/key generation only. Continuations never prove encryption.
//! Reuses the packing rule's exact AST grammar and scoped bijective matcher.
use super::*;
use oxc_span::GetSpan;

const PREPARE: &str = r#"function prepare(input){
let encoded=new TextEncoder().encode(input);let pending;
if(CAP_SUBTLE!=null){pending=CAP_SUBTLE.generateKey({name:"AES-GCM",length:128},true,["encrypt","decrypt"]).then(key=>key);}else{pending=null;}
if(pending!=null){return pending.then(CONTINUATION);}else{return null;}}
"#;
static SHELLS: LazyLock<Vec<Tree>> = LazyLock::new(|| {
    let direct = r#"pending=CAP_SUBTLE.generateKey({name:"AES-GCM",length:128},true,["encrypt","decrypt"]).then(key=>key);"#;
    let descriptor = r#"let service=CAP_SUBTLE;let algorithm={name:"AES-GCM"};
Object.defineProperty(algorithm,"length",{writable:true,configurable:true,enumerable:true,value:128});
pending=service.generateKey(algorithm,true,["encrypt","decrypt"]).then(key=>key);"#;
    vec![
        compile(PREPARE, false),
        compile(&PREPARE.replace(direct, descriptor), false),
    ]
});
static HOST: LazyLock<Tree> = LazyLock::new(|| {
    compile(
        r#"typeof global !== "undefined" && global != null && global.global === global ? global : typeof self !== "undefined" && self != null && self.self === self ? self : typeof window !== "undefined" && window != null && window.window === window ? window : void 0;"#,
        true,
    )
});
static GLOBAL_HOST: LazyLock<Tree> = LazyLock::new(|| compile("globalThis;", true));

// Only the final continuation is opaque. The generateKey continuation remains
// the exact identity arrow; all preparation statements and both null paths match.
fn owned_tree(model: &Model<'_, '_>, function: &Function<'_>) -> Option<Tree> {
    if function.r#async || function.generator {
        return None;
    }
    let body = function.body.as_ref()?;
    if !body.directives.is_empty() || body.statements.len() != 4 {
        return None;
    }
    let mut syntax = Syntax {
        scopes: model.scoping,
        remaining: 512,
    };
    let Statement::IfStatement(last) = &body.statements[3] else {
        return None;
    };
    let Statement::BlockStatement(yes) = &last.consequent else {
        return None;
    };
    let [Statement::ReturnStatement(ret)] = yes.body.as_slice() else {
        return None;
    };
    let Some(Expression::CallExpression(call)) = &ret.argument else {
        return None;
    };
    let [Argument::ArrowFunctionExpression(callback)] = call.arguments.as_slice() else {
        return None;
    };
    if callback.r#async {
        return None;
    }
    let mut statements = body.statements[..3]
        .iter()
        .map(|s| syntax.statement(s))
        .collect::<Option<Vec<_>>>()?;
    statements.push(node(
        "if",
        vec![
            syntax.expression(&last.test)?,
            node(
                "block",
                vec![node(
                    "statements",
                    vec![node(
                        "return",
                        vec![node(
                            format!("call:{}", call.optional),
                            vec![
                                syntax.expression(&call.callee)?,
                                Tree::Global("CONTINUATION".into()),
                            ],
                        )],
                    )],
                )],
            ),
            syntax.statement(last.alternate.as_ref()?)?,
        ],
    ));
    Some(node(
        "function",
        vec![
            syntax.params(&function.params)?,
            node("statements", statements),
        ],
    ))
}

// Resolve a single origin in a direct statement container. All value reads must
// follow initialization, or occur inside closures whose complete exposure chain
// follows it. This rejects early invocation, escaped/overwritten aliases, and
// conditional initialization while allowing recovered hoisted declarations.
fn ordered_origin<'a>(model: &Model<'_, 'a>, symbol: SymbolId) -> Option<&'a Expression<'a>> {
    let value = capture_value(model, symbol)?;
    let decl = model.scoping.symbol_declaration(symbol);
    let (statement, write_reference) = if model.initializer(symbol).is_some() {
        (model.nodes.parent_id(decl), None)
    } else {
        let reference = model
            .scoping
            .get_resolved_references(symbol)
            .find(|r| r.is_write())?;
        let assignment = model.nodes.parent_id(reference.node_id());
        let statement = model.nodes.parent_id(assignment);
        if !matches!(model.nodes.kind(statement), AstKind::ExpressionStatement(_)) {
            return None;
        }
        (statement, Some(reference.node_id()))
    };
    let container = model.nodes.parent_id(statement);
    if !matches!(
        model.nodes.kind(container),
        AstKind::BlockStatement(_) | AstKind::FunctionBody(_) | AstKind::Program(_)
    ) {
        return None;
    }
    let end = model.nodes.kind(statement).span().end;
    if !model
        .scoping
        .get_resolved_references(symbol)
        .all(|reference| {
            reference.is_write() && write_reference == Some(reference.node_id())
                || callable_origin_precedes(
                    model,
                    container,
                    end,
                    reference.node_id(),
                    write_reference,
                    0,
                    true,
                )
        })
    {
        return None;
    }
    Some(value)
}

fn origin_proven(model: &Model<'_, '_>, site: FunctionSite, matched: &mut Match) -> bool {
    let Some(subtle) = matched.captures.get("CAP_SUBTLE").copied() else {
        return false;
    };
    let Some(origin) = ordered_origin(model, subtle) else {
        return false;
    };
    let Some(origin) = (Syntax {
        scopes: model.scoping,
        remaining: 128,
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
    let Some(host) = ordered_origin(model, context) else {
        return false;
    };
    // Each supported ambient-host origin is exact and unshadowed. A random
    // immutable object with a crypto-named field is not native-origin proof.
    let Some(host) = (Syntax {
        scopes: model.scoping,
        remaining: 128,
    })
    .expression(host) else {
        return false;
    };
    host == *HOST || host == *GLOBAL_HOST
}

pub(super) fn evaluate(model: &Model<'_, '_>, site: FunctionSite, f: &Facts<'_>) -> Vec<Proposal> {
    if !f.members.contains("generateKey") || !f.members.contains("encode") {
        return vec![];
    }
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return vec![];
    };
    let Some(actual) = owned_tree(model, function) else {
        return vec![];
    };
    for shell in SHELLS.iter() {
        let mut matched = Match::default();
        if matched.tree(shell, &actual, model, site) && origin_proven(model, site, &mut matched) {
            return vec![proposal("prepareTextAndGenerateAesGcmKey", "owned_aes_key_preparation",
                "whole owned body encodes its input with unshadowed TextEncoder and null-guards the same stable native-host crypto.subtle capture; exact AES-GCM/128/true/encrypt,decrypt generation, identity first continuation and promise/null return flow; literal and complete own-data-descriptor options are separate proofs; final callback body supplies no parent evidence", f)];
        }
    }
    vec![]
}
