//! Finite ordered inline lease production, followed by the complete lifecycle.
//!
//! This is an additive fallback. The contract preserves every statement,
//! including scratch resets and post-consumption clobbers. Alpha equivalence
//! uses lexical SymbolIds; it is not text matching generated identifiers.
//! Capture readiness is bounded to an owning activation and one private factory boundary. In
//! particular, an ancestor's hoisted factory is not evidence that its captures
//! have been initialized. Native storage and the token consumer's purpose are
//! opaque: this rule names only the observed owned protocol.
use super::*;
use oxc_span::{GetSpan, Span};
use std::sync::OnceLock;

const NAME: &str = "initializePersistentLeaseAndModuleState";
const EVALUATOR: &str = "persistent_token_init_variants";
const GLOBALS: &[&str] = &[
    "Date",
    "Math",
    "performance",
    "Number",
    "crypto",
    "Uint32Array",
    "setTimeout",
];
const CONTRACT: &str = r#"function initialize() {
let key = context.persistentId.key;
let token = context.transactionToken;
try {
	stored = false;
	if (storage) {
		if (!storage.getItem(key)) {
			storage.setItem(key, token);
		}
		stored = true;
	}
} catch (e) {}
let random, mix;
try {
	if (storage) {
		leaseKey = "l_" + context.persistentId.key;
		let stamp = Date.now();
		mix = void 0;
		random = void 0;
		random = Math.random() * 4294967296 >>> 0;
		mix = 0;
		let clock = 0;
		try {
			clock = performance.now() >>> 0;
		} catch (e_2) {}
		let date = Number(new Date()) >>> 0;
		let cryptoWord = 0;
		try {
			cryptoWord = crypto.getRandomValues(new Uint32Array(1))[0];
		} catch (e_3) {}
		random = random ^ 1103515245;
		clock = clock === clock ? random ^ clock : random;
		date = date === date ? clock ^ date : clock;
		++counter;
		mix = (date ^ cryptoWord ^ counter) >>> 0;
		leaseValue = "" + stamp + mix;
		if (!storage.getItem(leaseKey)) {
			storage.setItem(leaseKey, leaseValue);
		}
	}
	random = 62004;
	mix = 2;
} catch (e_4) {}
tokenState = consumeToken(context.transactionToken);
continueInit();
try {
	modules.forEach(item => {
		item.init(makeStream());
		if (item.afterReady != null) {
			if (typeof setTimeout !== "function") {
				item.afterReady.call(void 0);
			} else if (deferReady) {
				setTimeout(item.afterReady, 0);
			} else if (readyQueue) {
				readyQueue.push(item.afterReady);
			}
		}
	});
} catch (e_5) {
	let record = {};
	record.message = "3.1:" + e_5;
	report(record);
}
}"#;

const GUARDED_CRYPTO: &str = r#"function initialize() {
let key = context.persistentId.key;
let token = context.transactionToken;
try {
	stored = false;
	if (storage) {
		if (!storage.getItem(key)) {
			storage.setItem(key, token);
		}
		stored = true;
	}
} catch (e) {}
let mix = context.persistentId.key;
try {
	if (storage) {
		mix = context.persistentId.key;
		leaseKey = "l_" + context.persistentId.key;
		let stamp = Date.now();
		let random = Math.random() * 4294967296 >>> 0;
		let clock = 0;
		try {
			clock = performance.now() >>> 0;
		} catch (e_2) {}
		let date = Number(new Date()) >>> 0;
		let cryptoWord = 0;
		try {
			cryptoWord = crypto.getRandomValues(new Uint32Array(1))[0];
		} catch (e_3) {}
		random = random ^ 1103515245;
		clock = clock === clock ? random ^ clock : random;
		date = date === date ? clock ^ date : clock;
		if (cryptoWord === cryptoWord) {
			mix = 3;
			cryptoWord = date ^ cryptoWord;
		} else {
			cryptoWord = date;
		}
		++counter;
		mix = (cryptoWord ^ counter) >>> 0;
		leaseValue = "" + stamp + mix;
		if (!storage.getItem(leaseKey)) {
			storage.setItem(leaseKey, leaseValue);
		}
	}
	mix = 3;
} catch (e_4) {}
tokenState = consumeToken(context.transactionToken);
continueInit();
try {
	modules.forEach(item => {
		item.init(makeStream());
		if (item.afterReady != null) {
			if (typeof setTimeout !== "function") {
				item.afterReady.call(void 0);
			} else if (deferReady) {
				setTimeout(item.afterReady, 0);
			} else if (readyQueue) {
				readyQueue.push(item.afterReady);
			}
		}
	});
} catch (e_5) {
	let record = {};
	record.message = "3.1:" + e_5;
	report(record);
}
}"#;

const GUARDED_WORDS: &str = r#"function initialize() {
let key = context.persistentId.key;
let token = context.transactionToken;
try {
	stored = false;
	if (storage) {
		if (!storage.getItem(key)) {
			storage.setItem(key, token);
		}
		stored = true;
	}
} catch (e) {}
let mix = context.persistentId.key;
let random;
try {
	if (storage) {
		mix = context.persistentId.key;
		leaseKey = "l_" + context.persistentId.key;
		let stamp = Date.now();
		random = void 0;
		random = Math.random() * 4294967296 >>> 0;
		let clock = 0;
		try {
			clock = performance.now() >>> 0;
		} catch (e_2) {}
		let date = Number(new Date()) >>> 0;
		let cryptoWord = 0;
		try {
			cryptoWord = crypto.getRandomValues(new Uint32Array(1))[0];
		} catch (e_3) {}
		mix = 1103515245;
		if (random === random) {
			mix = random ^ 1103515245;
			random = random ^ 1103515245;
		} else {
			random = 1103515245;
		}
		if (clock === clock) {
			mix = random ^ clock;
		} else {
			mix = random;
		}
		if (date === date) {
			mix = mix ^ date;
		}
		if (cryptoWord === cryptoWord) {
			mix = mix ^ cryptoWord;
		}
		++counter;
		random = (mix ^ counter) >>> 0;
		leaseValue = "" + stamp + random;
		if (!storage.getItem(leaseKey)) {
			storage.setItem(leaseKey, leaseValue);
		}
	}
	random = 149903;
	mix = 4;
} catch (e_4) {}
tokenState = consumeToken(context.transactionToken);
continueInit();
try {
	modules.forEach(item => {
		item.init(makeStream());
		if (item.afterReady != null) {
			if (typeof setTimeout !== "function") {
				item.afterReady.call(void 0);
			} else if (deferReady) {
				setTimeout(item.afterReady, 0);
			} else if (readyQueue) {
				readyQueue.push(item.afterReady);
			}
		}
	});
} catch (e_5) {
	let record = {};
	record.message = "3.1:" + e_5;
	report(record);
}
}"#;

fn canonical(source: &str) -> Option<String> {
    let allocator = Allocator::default();
    let program = crate::analysis::parse_program(&allocator, source).ok()?;
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

/// The two namespaces distinguish a local from a capture even when they have
/// identical spelling. Source-order numbering also preserves capture equality.
fn normalize(
    source: &str,
    scoping: &Scoping,
    nodes: &oxc_semantic::AstNodes<'_>,
    span: Span,
    template: bool,
) -> Option<(String, Vec<SymbolId>)> {
    let mut ids = BTreeMap::new();
    for node in nodes.iter().filter(|n| span.contains_inclusive(n.span())) {
        let (id_span, symbol, unresolved) = match node.kind() {
            AstKind::BindingIdentifier(id) => (id.span, Some(id.symbol_id.get()?), None),
            AstKind::IdentifierReference(id) => {
                let symbol = crate::analysis::identifier_symbol(id, scoping);
                if symbol.is_none() && GLOBALS.contains(&id.name.as_str()) {
                    continue;
                }
                if symbol.is_none() && !template {
                    return None;
                }
                (id.span, symbol, Some(id.name.as_str()))
            }
            AstKind::WithStatement(_) | AstKind::Class(_) | AstKind::Function(_) => return None,
            _ => continue,
        };
        let key = if let Some(s) = symbol {
            if !scoping.symbol_redeclarations(s).is_empty() {
                return None;
            }
            if span.contains_inclusive(nodes.get_node(scoping.symbol_declaration(s)).span()) {
                // An owned scratch value cannot escape to a sibling consumer.
                if scoping
                    .get_resolved_references(s)
                    .any(|r| !span.contains_inclusive(nodes.get_node(r.node_id()).span()))
                {
                    return None;
                }
                (false, format!("{s:?}"))
            } else {
                (true, format!("{s:?}"))
            }
        } else {
            (true, unresolved?.to_string())
        };
        ids.insert((id_span.start, id_span.end), (key, symbol));
    }
    let mut locals = BTreeMap::new();
    let mut captures = BTreeMap::new();
    let mut capture_symbols = Vec::new();
    let mut edits = Vec::new();
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

fn expected() -> &'static Vec<String> {
    static EXPECTED: OnceLock<Vec<String>> = OnceLock::new();
    EXPECTED.get_or_init(|| {
        [CONTRACT, GUARDED_CRYPTO, GUARDED_WORDS]
            .iter()
            .map(|contract| {
                let allocator = Allocator::default();
                let program = crate::analysis::parse_program(&allocator, contract)
                    .expect("inline contract parses");
                let (scoping, nodes) = crate::analysis::analyze(&program);
                let Statement::FunctionDeclaration(function) = &program.body[0] else {
                    unreachable!()
                };
                normalize(
                    contract,
                    &scoping,
                    &nodes,
                    function.body.as_ref().unwrap().span,
                    true,
                )
                .expect("inline contract normalizes")
                .0
            })
            .collect()
    })
}

fn direct_store(model: &Model<'_, '_>, statement: &Statement<'_>, symbol: SymbolId) -> bool {
    matches!(statement, Statement::ExpressionStatement(s) if matches!(s.expression.without_parentheses(),
        Expression::AssignmentExpression(a) if a.operator == AssignmentOperator::Assign
        && matches!(&a.left, AssignmentTarget::AssignmentTargetIdentifier(id)
            if crate::analysis::identifier_symbol(id, model.scoping) == Some(symbol))))
}

/// Only unconditional stores and a try with both normal and caught stores
/// establish readiness. A branch-only assignment never establishes it.
fn initializing_stores(
    model: &Model<'_, '_>,
    statement: &Statement<'_>,
    symbol: SymbolId,
) -> Option<Vec<Span>> {
    if direct_store(model, statement, symbol) {
        let Statement::ExpressionStatement(s) = statement else {
            unreachable!()
        };
        let Expression::AssignmentExpression(a) = s.expression.without_parentheses() else {
            unreachable!()
        };
        return Some(vec![a.left.span()]);
    }
    match statement {
        Statement::VariableDeclaration(v) => v
            .declarations
            .iter()
            .find(|d| {
                d.init.is_some()
                    && matches!(&d.id, BindingPattern::BindingIdentifier(id)
                if id.symbol_id.get() == Some(symbol))
            })
            .map(|d| vec![d.id.span()]),
        Statement::TryStatement(t) if t.finalizer.is_none() => {
            let [normal] = t.block.body.as_slice() else {
                return None;
            };
            let [caught] = t.handler.as_ref()?.body.body.as_slice() else {
                return None;
            };
            if !direct_store(model, normal, symbol) || !direct_store(model, caught, symbol) {
                return None;
            }
            let mut stores = initializing_stores(model, normal, symbol)?;
            stores.extend(initializing_stores(model, caught, symbol)?);
            Some(stores)
        }
        _ => None,
    }
}

/// First reference to a private hoisted function is an exposure boundary, not
/// its declaration. A sole reference bounds initial exposure only; aliases must
/// additionally satisfy the finite outer API path below.
fn sole_exposure(model: &Model<'_, '_>, site: FunctionSite) -> Option<Span> {
    let symbol = site.binding_symbol?;
    let refs: Vec<_> = model.scoping.get_resolved_references(symbol).collect();
    let [r] = refs.as_slice() else {
        return None;
    };
    (!r.is_write()).then(|| model.nodes.get_node(r.node_id()).span())
}

/// Admit only the observed private factory -> immutable alias -> constructor
/// call in a sibling -> directly returned outer init API. Inspect every alias
/// reference, including references in hoisted helpers, rather than treating the
/// assignment's source position as the time at which the factory runs.
fn factory_exposure(model: &Model<'_, '_>, factory: FunctionSite) -> Option<Span> {
    let exposure = sole_exposure(model, factory)?;
    let reference = model
        .nodes
        .iter()
        .find(|n| n.span() == exposure && matches!(n.kind(), AstKind::IdentifierReference(_)))?;
    let AstKind::AssignmentExpression(assign) = model.nodes.parent_kind(reference.id()) else {
        return None;
    };
    if assign.operator != AssignmentOperator::Assign
        || crate::analysis::expression_symbol(&assign.right, model.scoping)
            != factory.binding_symbol
    {
        return None;
    }
    let AssignmentTarget::AssignmentTargetIdentifier(alias) = &assign.left else {
        return None;
    };
    let alias = crate::analysis::identifier_symbol(alias, model.scoping)?;
    let owner = model.enclosing_function(factory.node_id)?;
    let AstKind::Function(function) = model.nodes.kind(owner.node_id) else {
        return None;
    };
    let statements = &function.body.as_ref()?.statements;
    let assignment = statements
        .iter()
        .find(|s| direct_store(model, s, alias) && s.span().contains_inclusive(exposure))?
        .span();
    let refs: Vec<_> = model.scoping.get_resolved_references(alias).collect();
    if refs.len() != 2 || refs.iter().filter(|r| r.is_write()).count() != 1 {
        return None;
    }
    let read = refs.iter().find(|r| !r.is_write())?;
    let AstKind::NewExpression(new) = model.nodes.parent_kind(read.node_id()) else {
        return None;
    };
    if crate::analysis::expression_symbol(&new.callee, model.scoping) != Some(alias) {
        return None;
    }
    let caller = model.enclosing_function(read.node_id())?;
    if caller.span == factory.span || model.enclosing_function(caller.node_id)?.span != owner.span {
        return None;
    }
    let caller_exposure = sole_exposure(model, caller)?;
    // Require the final outer statement itself to return the sibling entry.
    // A nested helper return cannot establish execution dominance.
    let Statement::ReturnStatement(ret) = statements.last()? else {
        return None;
    };
    if assignment.end >= ret.span.start || !ret.span.contains_inclusive(caller_exposure) {
        return None;
    }
    let Some(Expression::ObjectExpression(object)) = &ret.argument else {
        return None;
    };
    if !object.properties.iter().any(|p| {
        matches!(p,
        ObjectPropertyKind::ObjectProperty(p)
        if property_key_name(&p.key).as_deref() == Some("init")
        && crate::analysis::expression_symbol(&p.value, model.scoping) == caller.binding_symbol)
    }) {
        return None;
    }
    Some(exposure)
}

fn activation_proof(
    model: &Model<'_, '_>,
    site: FunctionSite,
    symbol: SymbolId,
    index: usize,
) -> Option<(Span, Span, Vec<Span>)> {
    let declaration = model.scoping.symbol_declaration(symbol);
    let AstKind::VariableDeclarator(d) = model.nodes.kind(declaration) else {
        return None;
    };
    let Some(decl_owner) = model.enclosing_function(declaration) else {
        return None;
    };
    let Some(child) = model.enclosing_function(site.node_id) else {
        return None;
    };
    if model.nodes.iter().any(|n| {
        decl_owner.span.contains_inclusive(n.span())
            && (matches!(n.kind(), AstKind::WithStatement(_))
                || matches!(n.kind(), AstKind::IdentifierReference(id) if id.name == "eval"))
    }) {
        return None;
    }
    // Walk only lexical owning activations. At each boundary the child must
    // remain private until its one source-indexed exposure. No source-position
    // assumption about a hoisted declaration substitutes for this check.
    let boundary = loop {
        if child.span == decl_owner.span {
            let Some(exposure) = sole_exposure(model, site) else {
                return None;
            };
            break exposure;
        }
        let Some(parent) = model.enclosing_function(child.node_id) else {
            return None;
        };
        if parent.span == decl_owner.span {
            let Some(exposure) = factory_exposure(model, child) else {
                return None;
            };
            break exposure;
        }
        // A second unproved factory layer is outside this finite production.
        return None;
    };
    // Find the concrete statement list containing exposure. Initialization in
    // the containing body or the same branch is unconditional on this path.
    let AstKind::Function(owner) = model.nodes.kind(decl_owner.node_id) else {
        return None;
    };
    let Some(body) = &owner.body else {
        return None;
    };
    let mut lists: Vec<&[Statement<'_>]> = vec![&body.statements];
    for node in model.nodes.iter().filter(|n| {
        decl_owner.span.contains_inclusive(n.span()) && n.span().contains_inclusive(boundary)
    }) {
        if let AstKind::BlockStatement(block) = node.kind() {
            lists.push(&block.body);
        }
    }
    let (initialized, stores) = lists.iter().find_map(|list| {
        list.iter().find_map(|s| {
            (s.span().end < boundary.start)
                .then(|| initializing_stores(model, s, symbol))
                .flatten()
                .map(|stores| (s.span(), stores))
        })
    })?;
    // All SymbolId writers participate, regardless of lexical owner or source
    // position. Try/catch stores are one ready epoch with two exclusive arms.
    // Protocol outputs are handled separately by the exact owned grammar.
    let closed = model.scoping.get_resolved_references(symbol).filter(|r| r.is_write()).all(|r| {
        let span = model.nodes.get_node(r.node_id()).span();
        // A statement can also declare deferred functions or execute other
        // assignments. Only the exact store in this activation is a ready
        // epoch; lexical containment does not establish execution ordering.
        if stores.contains(&span)
            && model.owning_site(r.node_id()).is_some_and(|f| f.span == decl_owner.span)
        { return true; }
        match index {
            // The shared entropy counter is intentionally mutable. Only ++
            // epochs preserve its numeric origin; arbitrary replacements fail.
            4 => matches!(model.nodes.parent_kind(r.node_id()), AstKind::UpdateExpression(u)
                if u.operator == oxc_syntax::operator::UpdateOperator::Increment),
            // Lifecycle state has a finite value domain, not an immutable
            // helper identity. true and null are the observed terminal epochs.
            11 | 12 => matches!(model.nodes.parent_kind(r.node_id()), AstKind::AssignmentExpression(a)
                if a.operator == AssignmentOperator::Assign && match (index, a.right.without_parentheses()) {
                    (11, Expression::BooleanLiteral(b)) => b.value,
                    (12, Expression::NullLiteral(_)) => true,
                    _ => false,
                }),
            _ => false,
        }
    });
    if !closed {
        return None;
    }
    Some((initialized, boundary, stores))
}

fn ready_at_exposure(model: &Model<'_, '_>, site: FunctionSite, captures: &[SymbolId]) -> bool {
    let Some(exposure) = sole_exposure(model, site) else {
        return false;
    };
    // The sole exposure must be a literal returned public init property.
    let Some(owner) = model.enclosing_function(site.node_id) else {
        return false;
    };
    let matching_return = model.nodes.iter().any(|n| {
        if !owner.span.contains_inclusive(n.span())
            || !n.span().contains_inclusive(exposure)
            || model
                .enclosing_function(n.id())
                .is_none_or(|f| f.span != owner.span)
        {
            return false;
        }
        let AstKind::ReturnStatement(ret) = n.kind() else {
            return false;
        };
        let Some(Expression::ObjectExpression(object)) = ret.argument.as_ref() else {
            return false;
        };
        object.properties.iter().any(|p| {
            matches!(p, ObjectPropertyKind::ObjectProperty(p)
            if property_key_name(&p.key).as_deref() == Some("init")
            && crate::analysis::expression_symbol(&p.value,model.scoping) == site.binding_symbol)
        })
    });
    if !matching_return {
        return false;
    }
    if model.nodes.iter().any(|n| {
        owner.span.contains_inclusive(n.span())
            && matches!(n.kind(),
        AstKind::IdentifierReference(id) if id.name == "eval")
    }) {
        return false;
    }
    // These captures are outputs, assigned before any use by the exact grammar.
    // All other captures need initialization before their owning exposure.
    captures.iter().enumerate().all(|(index, &symbol)| {
        if [1, 3, 5, 6].contains(&index) {
            return true;
        }
        activation_proof(model, site, symbol, index).is_some()
    })
}

pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    f: &Facts<'_>,
    _proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return Vec::new();
    };
    if function.r#async
        || function.generator
        || !function.params.items.is_empty()
        || function.params.rest.is_some()
    {
        return Vec::new();
    }
    let Some(body) = &function.body else {
        return Vec::new();
    };
    if body.span.size() > 8000 || !model.exact(body.span).contains("persistentId") {
        return Vec::new();
    }
    let Some((grammar, captures)) =
        normalize(model.source, model.scoping, model.nodes, body.span, false)
    else {
        return Vec::new();
    };
    if !expected().contains(&grammar)
        || captures.len() != 14
        || !ready_at_exposure(model, site, &captures)
    {
        return Vec::new();
    }
    let roles = [
        "context",
        "storedFlag",
        "opaqueStorageProtocol",
        "leaseKey",
        "counter",
        "leaseValue",
        "tokenState",
        "opaqueTokenConsumer",
        "continuation",
        "modules",
        "opaqueStreamFactory",
        "deferFlag",
        "readyQueue",
        "opaqueErrorReporter",
    ];
    let captures_json: Vec<_> = captures.iter().enumerate().map(|(index, &symbol)| {
    let declaration = model.nodes.get_node(model.scoping.symbol_declaration(symbol)).span();
    let readiness = activation_proof(model,site,symbol,index).map(|(init,boundary,stores)|
        serde_json::json!({"initialization_statement":{"start":init.start,"end":init.end},
            "activation_exposure":{"start":boundary.start,"end":boundary.end}, "initializing_stores":stores.iter().map(|s| serde_json::json!({"start":s.start,"end":s.end})).collect::<Vec<_>>()}));
    // Include every resolved use/write, even in a deferred nested owner.
    // Keep exact stores separate from the containing statement's diagnostic span.
    let references: Vec<_> = model.scoping.get_resolved_references(symbol).map(|r| {
        let span = model.nodes.get_node(r.node_id()).span();
        let owner = model.owning_site(r.node_id()).map(|f|
            serde_json::json!({"start":f.span.start,"end":f.span.end}));
        serde_json::json!({"start":span.start,"end":span.end,
            "read":r.is_read(),"write":r.is_write(),"owner":owner})
    }).collect();
    let uses: Vec<_> = model.scoping.get_resolved_references(symbol).filter_map(|r| {
        let span=model.nodes.get_node(r.node_id()).span();
        site.span.contains_inclusive(span).then(|| serde_json::json!({
            "span":{"start":span.start,"end":span.end}, "read":r.is_read(),"write":r.is_write()}))
    }).collect();
    serde_json::json!({"role":roles[index],"binding":model.binding_ref(symbol),
        "declaration":{"start":declaration.start,"end":declaration.end},
        "initialization":readiness,"output_assigned_by_grammar":([1,3,5,6].contains(&index)),"owned_uses":uses,"all_writers":references.iter().filter(|r| r["write"] == true).collect::<Vec<_>>(),"all_references":references,"epoch_policy":match index {1|3|5|6=>"owned grammar output",4=>"numeric increment epochs",11=>"false to true lifecycle",12=>"array to null lifecycle",_=>"single ready epoch; all SymbolId writers closed"}})
}).collect();
    let typed_proof = serde_json::json!({"production":"finite_ordered_inline_lease",
    "full_declaration_span":{"start":site.span.start,"end":site.span.end},
    "captures":captures_json,"consumer_purpose":"opaque; no decoder identity asserted",
    "storage_purpose":"opaque protocol; no nativeStorage identity asserted",
    "synchronous_callback":"afterReady.call(void 0) is preserved and reachable only after init exposure",
    "seed_epoch":"final unsigned XOR reaches lease concatenation/store before post-store scratch clobber"});
    let mut result = proposal(NAME, EVALUATOR,
        "whole ordered inline lease/lifecycle grammar with lexical capture equality: token stored under persistentId.key; l_ lease consumes Date.now and the final random/performance/date/crypto/counter XOR before later scratch clobbers; opaque captured token consumer and continuation called once; exact module init, synchronous afterReady.call / timer / queue alternatives and 3.1 catch; all captures initialized in the owning activation before the sole returned init exposure; no native-storage or decoder identity asserted", f);
    result
        .explanation
        .push(format!("capture-init-use-proof:{}", typed_proof));
    vec![result]
}
