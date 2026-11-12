//! Complete event-state storage protocols. Captured transformation functions
//! remain opaque: no encryption, native storage, or helper purpose is inferred.
//! The closed grammar preserves calls, receivers, catches, literal operators,
//! mutation order and every nested continuation. Only lexical symbols and the
//! storage-key literal (joined across get/set) are parameters of the grammar.
use super::*;
use oxc_span::{GetSpan, Span};
use std::sync::OnceLock;

const GLOBALS: &[&str] = &["Object", "JSON", "Promise", "Uint8Array", "Number"];

fn canonical(source: &str) -> Option<String> {
    let arena = Allocator::default();
    let program = crate::analysis::parse_program(&arena, source).ok()?;
    Some(
        oxc_codegen::Codegen::new()
            .with_options(oxc_codegen::CodegenOptions {
                comments: oxc_codegen::CommentOptions::disabled(),
                ..Default::default()
            })
            .build(&program)
            .code,
    )
}

// Keep local and captured namespaces separate and join every read/write by
// SymbolId. Templates use unresolved CAP_* parameters; actual inputs never do.
fn normalize(
    source: &str,
    scoping: &Scoping,
    nodes: &oxc_semantic::AstNodes<'_>,
    span: Span,
    template: bool,
) -> Option<(String, Vec<SymbolId>)> {
    let mut ids = BTreeMap::new();
    let mut edits = Vec::new();
    let mut storage_key: Option<String> = None;
    for n in nodes.iter().filter(|n| span.contains_inclusive(n.span())) {
        let (token, symbol, unresolved) = match n.kind() {
            AstKind::BindingIdentifier(i) => (i.span, Some(i.symbol_id.get()?), None),
            AstKind::IdentifierReference(i) => {
                let s = crate::analysis::identifier_symbol(i, scoping);
                if s.is_none() && GLOBALS.contains(&i.name.as_str()) {
                    continue;
                }
                if s.is_none() && !(template && i.name.starts_with("CAP_")) {
                    return None;
                }
                (i.span, s, Some(i.name.to_string()))
            }
            AstKind::NumericLiteral(literal) => {
                // Recovered slot reuse can end in a dead numeric store. Only
                // that local store's value varies: retain the assignment and
                // require every other reference to precede it. In particular,
                // a callback reading the slot makes this proof fail.
                if let AstKind::AssignmentExpression(a) = nodes.parent_kind(n.id()) {
                    if a.operator.as_str() == "=" && a.right.span() == literal.span {
                        if let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left {
                            if let Some(s) = crate::analysis::identifier_symbol(id, scoping) {
                                let declaration = scoping.symbol_declaration(s);
                                let local = span.contains_inclusive(nodes.kind(declaration).span());
                                let mutable = matches!(nodes.parent_kind(declaration), AstKind::VariableDeclaration(d)
                                    if matches!(d.kind, VariableDeclarationKind::Let | VariableDeclarationKind::Var));
                                if local
                                    && mutable
                                    && literal.value.is_finite()
                                    && scoping.symbol_redeclarations(s).is_empty()
                                    && scoping.get_resolved_references(s).all(|r| {
                                        let reference = nodes.kind(r.node_id()).span();
                                        span.contains_inclusive(reference)
                                            && (reference == id.span
                                                || reference.end <= a.span.start)
                                    })
                                {
                                    edits.push((literal.span.start, literal.span.end, "0".into()));
                                }
                            }
                        }
                    }
                }
                continue;
            }
            AstKind::StringLiteral(literal) => {
                // Key variability is allowed only at the exact storage call
                // argument; all diagnostic strings and other values stay exact.
                if let AstKind::CallExpression(c) = nodes.parent_kind(n.id()) {
                    if let Expression::StaticMemberExpression(method) = &c.callee {
                        if matches!(method.property.name.as_str(), "getItem" | "setItem")
                            && c.arguments
                                .first()
                                .is_some_and(|a| a.span() == literal.span)
                        {
                            if storage_key
                                .as_ref()
                                .is_some_and(|k| k != literal.value.as_str())
                            {
                                return None;
                            }
                            storage_key = Some(literal.value.to_string());
                            edits.push((
                                literal.span.start,
                                literal.span.end,
                                "\"STORAGE_KEY\"".into(),
                            ));
                        }
                    }
                }
                continue;
            }
            AstKind::Function(f) if f.span == span => continue,
            AstKind::WithStatement(_) | AstKind::Class(_) | AstKind::Function(_) => return None,
            _ => continue,
        };
        let key = if let Some(s) = symbol {
            if !scoping.symbol_redeclarations(s).is_empty() {
                return None;
            }
            let local = span.contains_inclusive(nodes.kind(scoping.symbol_declaration(s)).span());
            let own_binding = matches!(nodes.kind(scoping.symbol_declaration(s)), AstKind::Function(f) if f.span == span);
            if own_binding && scoping.get_resolved_references(s).any(|r| r.is_write()) {
                return None;
            }
            if local
                && !own_binding
                && scoping
                    .get_resolved_references(s)
                    .any(|r| !span.contains_inclusive(nodes.kind(r.node_id()).span()))
            {
                return None;
            }
            (!local, format!("{s:?}"))
        } else {
            (true, unresolved?)
        };
        ids.insert((token.start, token.end), (key, symbol));
    }
    let mut locals = BTreeMap::new();
    let mut captures = BTreeMap::new();
    let mut capture_symbols = Vec::new();
    for ((start, end), ((capture, key), symbol)) in ids {
        let names = if capture { &mut captures } else { &mut locals };
        let next = names.len();
        let index = *names.entry(key).or_insert_with(|| {
            if capture {
                if let Some(s) = symbol {
                    capture_symbols.push(s);
                }
            }
            next
        });
        edits.push((
            start,
            end,
            format!("__{}{}", if capture { "capture" } else { "local" }, index),
        ));
    }
    edits.sort_by_key(|e| (e.0, e.1));
    let mut text = source
        .get(span.start as usize..span.end as usize)?
        .to_string();
    for (start, end, replacement) in edits.into_iter().rev() {
        text.replace_range(
            (start - span.start) as usize..(end - span.start) as usize,
            &replacement,
        );
    }
    Some((canonical(&text)?, capture_symbols))
}

// Build variants from shared typed protocol parts. A recovered assignment into
// a diagnostic record is kept as such; no evaluation or coercion is discarded.
fn diagnostic(message: &str, error: &str, split: bool, reverse: bool) -> String {
    let stack = if reverse {
        format!("{error}+\"\"")
    } else {
        format!("\"\"+{error}")
    };
    if split {
        format!("let record={{message:\"{message}\",stack:void 0}};record.stack={stack};CAP_REPORT(record);")
    } else {
        format!("CAP_REPORT({{message:\"{message}\",stack:{stack}}});")
    }
}
fn read_storage(code: &str) -> String {
    format!("let stored;try{{stored=CAP_CONTEXT.localStorage.getItem(\"STORAGE_KEY\");}}catch(e){{{}return void 0;}}", diagnostic(code,"e",false,false))
}
// Two count initializations are explicit alternatives, preserving their real
// effect order. Remaining differences are lexical bindings, not input IDs.
fn history_contract(inline_count: bool) -> String {
    let count = if inline_count { "path?1:0" } else { "void 0" };
    let assign = if inline_count {
        ""
    } else {
        "entry.tc=path?1:0;"
    };
    format!(
        r#"function protocol(eventType,url){{
let history=CAP_EVENTS.et;
let elapsed=Number(new CAP_CLOCK())-CAP_START;
let hours=Number((Number(new CAP_CLOCK())-CAP_ORIGIN)/1e3/60/60);
CAP_EVENTS.lit=eventType;CAP_EVENTS.tlr=elapsed;
let protocol="";let hostname="";let port="";let path="";
if(url){{let parsed=CAP_PARSE(url);
hostname=parsed.hostname===CAP_HOST?"":parsed.hostname;
protocol=parsed.protocol===CAP_PROTOCOL?"":parsed.protocol;
port=parsed.port===CAP_PORT?"":parsed.port;
let length=parsed.pathname.length;
path=(length<70?parsed.pathname:parsed.pathname.substring(0,65)+"-"+length+"-"+parsed.pathname.substring(length-7))||"";
}}
let entry={{p1:protocol,p2:hostname,p3:port,p4:path,tc:{count},ty:void 0,lst:void 0}};
{assign}let types=[eventType];entry.ty=types;let times=[elapsed];entry.lst=times;
let index=history.findIndex(item=>item.p1.length===0&&item.p2.length===0&&item.p3.length===0&&item.p4===path||item.p1===protocol&&item.p2===hostname&&item.p3===port&&item.p4===path);
if(index===-1){{history.push(entry);}}else{{history[index].ty.push(eventType);history[index].lst.push(elapsed);
if(path){{history[index].tc=Number(history[index].tc)+1;}}
if(history[index].ty.length>10){{history[index].ty.shift();history[index].lst.shift();}}
}}
if(history.length>1){{history.shift();}}CAP_EVENTS.et=history;
if(path){{CAP_EVENTS.c=Number(CAP_EVENTS.c)+1;}}
CAP_EVENTS.top=hours;CAP_EVENTS.lvTop=Number((Number(new CAP_CLOCK())-CAP_PREVIOUS)/1e3/60/60);
CAP_PREVIOUS=Number(new CAP_CLOCK());
if(path){{CAP_STATE.a.c=Number(CAP_STATE.a.c)+1;}}
CAP_STATE.a.ts=CAP_STATE.a.ts+CAP_EVENTS.lvTop;CAP_STATE.b=CAP_STATE.a.c;
CAP_REFRESH();let pending=CAP_TRANSFORM(JSON.stringify(CAP_STATE.a));
if(pending!=null){{pending.then(value=>new Uint8Array(value).toString()).then(value=>{{return CAP_CONTEXT.localStorage.setItem("STORAGE_KEY",JSON.stringify({{a:value,b:CAP_STATE.a.c}}));}}).catch(error=>{{return CAP_REPORT({{message:"IM6",stack:""+error}});}});}}
}}"#
    )
}

fn contracts() -> &'static Vec<(String, &'static str)> {
    static CONTRACTS: OnceLock<Vec<(String, &'static str)>> = OnceLock::new();
    CONTRACTS.get_or_init(|| {
        let mut result=Vec::new();
        for split in [false,true] { for reverse in [false,true] {
            // Loading one aggregate record: optional transformation of field a,
            // then field b copied only after that promise has fulfilled.
            let aggregate=format!("function protocol(){{{}if(stored!=null){{let record;try{{record=JSON.parse(stored);}}catch(e2){{{}return void 0;}}if(record.a){{CAP_TRANSFORM(record.a).then(value=>{{CAP_STATE.a=value;if(record.b){{CAP_STATE.b=record.b;}}}}).catch(error=>{{return CAP_REPORT({{message:\"IM8\",stack:\"\"+error}});}});}}}}}}",read_storage("IM9"),diagnostic("IM10","e2",split,reverse));
            result.push((aggregate,"restoreAggregateEventState"));
            // Restore a map entry only for each fulfilled indexed result; never
            // infer what the opaque per-entry transformation computes.
            let map=format!("function protocol(){{{}let pending=stored;if(pending!=null){{try{{CAP_ENCODED=JSON.parse(stored);}}catch(e2){{{}return void 0;}}CAP_DECODED=Object.create(null);let keys=Object.keys(CAP_ENCODED);pending=keys.map(key=>CAP_TRANSFORM(CAP_ENCODED[key]));Promise.allSettled(pending).then(results=>{{return results.map((item,index)=>{{if(item.status===\"fulfilled\"){{CAP_DECODED[keys[index]]=item.value;}}else{{CAP_REPORT({{message:\"IM4\",stack:\"\"}});}}}});}}).catch(error=>{{return CAP_REPORT({{message:\"IM4\",stack:\"\"+error}});}});}}}}",read_storage("IM3"),diagnostic("IM10","e2",split,reverse));
            result.push((map,"restoreSerializedEventMap"));
            for alias in [false,true] {
                let prefix=read_storage("IM5");
                let setup=if alias {"let pending=stored;"} else {""};
                let guard=if alias {"pending"} else {"stored"};
                let transform=if alias {"let serialized=JSON.stringify(CAP_EVENTS);pending=CAP_TRANSFORM(serialized);"} else {"let pending=CAP_TRANSFORM(JSON.stringify(CAP_EVENTS));"};
                let save=format!("function protocol(){{let record=Object.create(null);{prefix}{setup}if({guard}!=null){{try{{record=JSON.parse(stored);}}catch(e2){{{}return void 0;}}if(Object.keys(record).length>0){{CAP_ENCODED=record;}}}}if(Object.keys(CAP_EVENTS).length!==0){{{transform}if(pending!=null){{pending.then(value=>{{record[CAP_INDEX]=new Uint8Array(value).toString();CAP_ENCODED=record;CAP_ENCODED=Object.fromEntries(Object.entries(CAP_ENCODED).slice(-1));CAP_CONTEXT.localStorage.setItem(\"STORAGE_KEY\",JSON.stringify(CAP_ENCODED));}}).catch(error=>{{return CAP_REPORT({{message:\"IM2\",stack:\"\"+error}});}});}}}}}}",diagnostic("IM10","e2",split,reverse));
                result.push((save,"saveLatestSerializedEventMap"));
            }
            // Distinct recovered layout: one local is reused for the read
            // guard and serialized input; the promise has its own binding.
            // Its terminal numeric store is retained and normalized only when
            // proven dead above. Byte conversion may have one local alias.
            for converted_alias in [false,true] {
                let converted=if converted_alias {
                    "let bytes=new Uint8Array(value).toString();record[CAP_INDEX]=bytes;"
                } else {
                    "record[CAP_INDEX]=new Uint8Array(value).toString();"
                };
                let save=format!("function protocol(){{let record=Object.create(null);{}let shared=stored;if(shared!=null){{try{{record=JSON.parse(stored);}}catch(e2){{{}return void 0;}}if(Object.keys(record).length>0){{CAP_ENCODED=record;}}}}if(Object.keys(CAP_EVENTS).length!==0){{shared=JSON.stringify(CAP_EVENTS);let pending=CAP_TRANSFORM(shared);if(pending!=null){{shared=0;pending.then(value=>{{{converted}CAP_ENCODED=record;CAP_ENCODED=Object.fromEntries(Object.entries(CAP_ENCODED).slice(-1));CAP_CONTEXT.localStorage.setItem(\"STORAGE_KEY\",JSON.stringify(CAP_ENCODED));}}).catch(error=>{{return CAP_REPORT({{message:\"IM2\",stack:\"\"+error}});}});}}}}}}",read_storage("IM5"),diagnostic("IM10","e2",split,reverse));
                result.push((save,"saveLatestSerializedEventMap"));
            }
        }}
        result.push((history_contract(false),"recordAndPersistBoundedEventHistory"));
        result.push((history_contract(true),"recordAndPersistBoundedEventHistory"));
        result.into_iter().map(|(text,name)| {
            let arena=Allocator::default();
            let program=crate::analysis::parse_program(&arena,&text).expect("storage protocol parses");
            let (scoping,nodes)=crate::analysis::analyze(&program);
            let Statement::FunctionDeclaration(f)=&program.body[0] else { unreachable!() };
            (normalize(&text,&scoping,&nodes,f.span,true).expect("storage protocol normalizes").0,name)
        }).collect()
    })
}

fn captures_valid(model: &Model<'_, '_>, body: Span, captures: &[SymbolId]) -> bool {
    for s in captures {
        let node = model.scoping.symbol_declaration(*s);
        if body.contains_inclusive(model.nodes.kind(node).span())
            || !model.scoping.symbol_redeclarations(*s).is_empty()
        {
            return false;
        }
        if !matches!(model.nodes.kind(node),AstKind::VariableDeclarator(v) if matches!(&v.id,BindingPattern::BindingIdentifier(i) if i.symbol_id.get()==Some(*s)))
        {
            return false;
        }
        let declaration = model.nodes.parent_id(node);
        if !matches!(
            model.nodes.kind(declaration),
            AstKind::VariableDeclaration(_)
        ) || matches!(
            model.nodes.parent_kind(declaration),
            AstKind::ForInStatement(_) | AstKind::ForOfStatement(_)
        ) || model.scoping.symbol_span(*s).end >= body.start
        {
            return false;
        }
    }
    // Captured direct callees are opaque but independently callable and ready.
    // This never uses their bodies as evidence for the storage role.
    model
        .nodes
        .iter()
        .filter(|n| body.contains_inclusive(n.span()))
        .all(|n| {
            let AstKind::CallExpression(c) = n.kind() else {
                return true;
            };
            let Expression::Identifier(i) = &c.callee else {
                return true;
            };
            let Some(s) = crate::analysis::identifier_symbol(i, model.scoping) else {
                return true;
            };
            !captures.contains(&s) || stable_function(model, &c.callee, 0).is_some()
        })
}

pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    facts: &Facts<'_>,
) -> Vec<Proposal> {
    // Facts belong to the immediate function; history storage is in an
    // owned promise continuation and is checked by the complete grammar.
    let storage = facts.members.contains("getItem") && facts.members.contains("localStorage");
    let history = facts.members.contains("findIndex")
        && facts.writes.contains("et")
        && facts.writes.contains("lvTop");
    if !storage && !history {
        return vec![];
    }
    let AstKind::Function(f) = model.nodes.kind(site.node_id) else {
        return vec![];
    };
    let Some(body) = f.body.as_ref() else {
        return vec![];
    };
    if f.r#async
        || f.generator
        || !matches!(f.params.items.len(), 0 | 2)
        || f.params.rest.is_some()
        || !body.directives.is_empty()
    {
        return vec![];
    }
    let Some((actual, captures)) =
        normalize(model.source, model.scoping, model.nodes, f.span, false)
    else {
        return vec![];
    };
    if !captures_valid(model, f.span, &captures) {
        return vec![];
    }
    for (expected, name) in contracts() {
        if &actual == expected {
            return vec![proposal(name,"persistent_event_state_storage","complete ordered storage read/JSON/error protocol and every owned promise continuation; lexical binding identities, state writes, diagnostic effects, exact receivers and calls preserved; transformation and report callees independently stable and initialized; no native storage or cryptographic helper purpose inferred",facts)];
        }
    }
    vec![]
}
