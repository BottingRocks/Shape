use super::*;
#[test]
#[ignore = "requires private source and trusted initializer paths; static only"]
fn private_instance_readiness_and_mutation_refusal() {
    use crate::signals::extract::{
        encoding::extract_encoding_properties, wiring::extract_program_wiring,
    };
    let source = std::fs::read_to_string(
        std::env::var_os("ALGORITHM_CERTIFICATION_SOURCE").expect("source path"),
    )
    .expect("read source");
    let initializer = std::fs::read_to_string(
        std::env::var_os("ALGORITHM_INITIALIZER_SOURCE").expect("initializer path"),
    )
    .expect("read initializer");
    let session = crate::signals::payload::session_from_initializer(&initializer)
        .unwrap_or_else(|_| panic!("initializer extraction failed"));
    let context = lifecycle::InitializationContext::from_initializer(
        &initializer,
        &session.transaction_token,
    )
    .unwrap_or_else(|_| panic!("initializer context failed"));
    let encoding = extract_encoding_properties(&source)
        .unwrap_or_else(|_| panic!("encoding extraction failed"));
    let wiring =
        extract_program_wiring(&source).unwrap_or_else(|_| panic!("wiring extraction failed"));
    let mut contracts: crate::signals::extract::contracts::SignalContractReport =
        serde_json::from_str(
            &std::fs::read_to_string(
                std::env::var_os("ALGORITHM_CONTRACTS_SOURCE").expect("contracts path"),
            )
            .expect("read contracts"),
        )
        .unwrap_or_else(|_| panic!("contract metadata failed"));
    assert_eq!(contracts.source.sha256, digest(source.as_bytes()));
    for contract in &mut contracts.signals {
        if contract.descriptor == 75 || contract.descriptor == 9 {
            for emission in &mut contract.emissions {
                bind_private_instance(&source, &encoding, emission);
            }
        }
    }
    let algorithms = certify(&source, &encoding, &wiring);
    let mut counts = BTreeMap::new();
    let mut ready = 0;
    let mut target = None;
    for contract in &contracts.signals {
        if contract.descriptor != 75 && contract.descriptor != 9 {
            continue;
        }
        for emission in &contract.emissions {
            match algorithms
                .certify_stream_instance(&source, &encoding, &wiring, emission, &context)
            {
                Ok(proof) => {
                    ready += 1;
                    if contract.descriptor == 75 {
                        target = Some(proof);
                    }
                }
                Err(e) => {
                    *counts.entry(e.0).or_insert(0) += 1;
                }
            }
        }
    }
    println!("source-backed ready instances: {ready}; refusals: {counts:?}");
    let proof = target.expect(
        "required previously supported instance must have a positive source readiness proof",
    );
    assert!(proof.body.codec.is_some());
    let span = proof.hash_assignment;
    let state = wiring
        .public_init
        .as_ref()
        .unwrap()
        .token
        .as_ref()
        .unwrap()
        .state_binding
        .as_ref()
        .unwrap();
    let mut changed = source.clone();
    changed.insert_str(span.end as usize, &format!("{}^=1;", state.name));
    let changed_encoding =
        extract_encoding_properties(&changed).unwrap_or_else(|_| panic!("mutated encoding failed"));
    let changed_wiring =
        extract_program_wiring(&changed).unwrap_or_else(|_| panic!("mutated wiring failed"));
    let changed_algorithms = certify(&changed, &changed_encoding, &changed_wiring);
    // The mutation follows the target's function definitions; its concrete
    // source spans and prefix binding therefore remain unchanged.
    let emission = &contracts
        .signals
        .iter()
        .find(|c| c.descriptor == 75)
        .unwrap()
        .emissions[0];
    assert!(changed_algorithms
        .certify_stream_instance(
            &changed,
            &changed_encoding,
            &changed_wiring,
            emission,
            &context
        )
        .is_err());
    println!(
        "private mutation refused; required instance ready; hash/integer/string: {:?}",
        [
            algorithms.hash.codec.is_some(),
            algorithms.integer.codec.is_some(),
            algorithms.string.codec.is_some()
        ]
    );
}

fn lifecycle_source() -> String {
    let base = super::tests::session(include_str!("reference/string.js"));
    let start = base
        .find("tokenState=hashAlias(context.transactionToken);")
        .unwrap();
    format!(
        "{}{}",
        &base[..start],
        r#"
    function group(emitters, context, onInit) {
        onInit(function callback(){
            let seed=7;let b=[seed>>>24,seed>>16&255,seed>>8&255,seed&255];
            let mask=streamFactory(seed^17);
            emitters.next([[],b]);
        });
    }
    let producers=[group];
    function coordinator(producer, context){let stored;producer(null,context,function(cb){stored=cb;});stored();}
    function start(){producers.forEach(function(producer){coordinator(producer,null);});}
    function SessionCtor(token){
        let context={transactionToken:token};
        function initialize(){tokenState=hashAlias(context.transactionToken);start();}
        return {init:initialize};
    }
    function publicEntry(token){let created=new SessionCtor(token);created.init();}
    return {init:publicEntry};
    }"#
    )
}
fn lifecycle_fixture(
    source: &str,
) -> (
    AlgorithmReport,
    EncodingProperties,
    WiringProperties,
    crate::signals::extract::contracts::EmissionContract,
) {
    use crate::signals::{
        extract::{
            contracts::*,
            layout::{EmissionVariant, ObserverRoute},
            wiring::*,
        },
        schema::FunctionRef,
    };
    let (encoding, mut wiring) = super::tests::fixture_inputs(source);
    let arena = Allocator::default();
    let parsed = Parser::new(&arena, source, SourceType::mjs()).parse();
    let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
    let function = |name: &str| {
        semantic
            .nodes()
            .iter()
            .find_map(|n| match n.kind() {
                AstKind::Function(f) if f.id.as_ref().is_some_and(|id| id.name == name) => Some(f),
                _ => None,
            })
            .unwrap()
    };
    let reference = |f: &Function<'_>| FunctionRef {
        binding: None,
        name_label: None,
        span: f.span.into(),
        body: f.body.as_ref().map(|b| b.span.into()),
        source: None,
    };
    let constructor = semantic
        .nodes()
        .iter()
        .find_map(|n| match n.kind() {
            AstKind::NewExpression(c) if matches!(&c.callee,Expression::Identifier(id) if id.name=="SessionCtor") => Some(c.span),
            _ => None,
        })
        .unwrap();
    let public = wiring.public_init.as_mut().unwrap();
    public.function = reference(function("publicEntry"));
    public.constructor_call_span = constructor.into();
    let coord = function("coordinator");
    let coord_formal = match &coord.params.items[0].pattern {
        BindingPattern::BindingIdentifier(b) => b.symbol_id.get().unwrap(),
        _ => panic!("formal"),
    };
    let invocation=semantic.nodes().iter().find_map(|n|match n.kind(){AstKind::CallExpression(c) if matches!(&c.callee,Expression::Identifier(id) if id.reference_id.get().and_then(|r|semantic.scoping().get_reference(r).symbol_id())==Some(coord_formal))=>Some(c),_=>None}).unwrap();
    wiring.groups = vec![GroupRouteWiring {
        group_index: 0,
        function: reference(function("group")),
        row: vec![0],
        routing_selector: false,
        row_span: SourceSpan::new(0, 0),
        selector_span: SourceSpan::new(0, 0),
        invocation: Some(CallbackInvocationWiring {
            span: invocation.span.into(),
            callee: String::new(),
            argument_count: invocation.arguments.len(),
            arguments: vec![],
        }),
        lifecycle: LifecycleWiring {
            registrations: vec![],
            body_emissions: vec![],
            evidence: vec![],
        },
        local_emitters: vec![],
        evidence: vec![],
    }];
    let find_symbol = |name: &str| {
        semantic
            .scoping()
            .symbol_ids()
            .find(|s| semantic.scoping().symbol_name(*s) == name)
            .unwrap()
    };
    let seed: SourceSpan = semantic.scoping().symbol_span(find_symbol("seed")).into();
    let prefix = semantic
        .nodes()
        .iter()
        .find_map(|n| match n.kind() {
            AstKind::ArrayExpression(a) if a.elements.len() == 4 => Some(a.span),
            _ => None,
        })
        .unwrap();
    let instance=semantic.nodes().iter().find_map(|n|match n.kind(){AstKind::CallExpression(c)if matches!(&c.callee,Expression::Identifier(i)if i.name=="streamFactory")=>Some(c.span),_=>None}).unwrap();
    let site=semantic.nodes().iter().find_map(|n|match n.kind(){AstKind::CallExpression(c)if matches!(&c.callee,Expression::StaticMemberExpression(m)if m.property.name=="next")=>Some(c.span),_=>None}).unwrap();
    let emission = EmissionContract {
        site: site.into(),
        forwarding_sites: vec![],
        quote: String::new(),
        route: ObserverRoute::Direct,
        phase: None,
        payload: EmissionVariant::RawBuffer,
        stream: Some(StreamContract {
            argument: String::new(),
            seed_word: None,
            salt: None,
            salt_span: None,
            instance: Some(instance.into()),
            seed_binding: Some(seed),
        }),
        rendered: String::new(),
        grammar: GrammarNode::RawPrefix(PrefixNode {
            span: prefix,
            spelling: PrefixSpelling::ArrayLiteral,
            seed_word: String::new(),
            elements: vec![],
            seed_binding: Some(seed),
        }),
    };
    let algorithms = certify(source, &encoding, &wiring);
    (algorithms, encoding, wiring, emission)
}
#[test]
fn returned_initializer_and_deferred_registration_are_ready() {
    let source = lifecycle_source();
    let (algorithms, encoding, wiring, emission) = lifecycle_fixture(&source);
    let context = lifecycle::InitializationContext::from_initializer(
        "e.detail.init('synthetic-token');",
        "synthetic-token",
    )
    .unwrap();
    assert!(algorithms.hash.codec.is_some());
    let proof = algorithms
        .certify_stream_instance(&source, &encoding, &wiring, &emission, &context)
        .unwrap();
    assert!(proof.body.codec.is_some());
    assert!(!proof.activation.is_empty());
    let mut tampered = proof.clone();
    tampered.hash_assignment.end += 1;
    assert!(algorithms
        .verify_stream_readiness(&tampered, &source, &encoding, &wiring, &emission, &context)
        .is_err());
    let deferred = source.replace(";start();", ";later();").replace(
        "function SessionCtor",
        "function later(){start();} function SessionCtor",
    );
    let (a, e, w, m) = lifecycle_fixture(&deferred);
    assert!(a
        .certify_stream_instance(&deferred, &e, &w, &m, &context)
        .is_ok());
    let callback_alias = source.replace(
        "return {init:initialize};",
        "let initAlias=initialize;return {init:initAlias};",
    );
    let (a, e, w, m) = lifecycle_fixture(&callback_alias);
    assert!(a
        .certify_stream_instance(&callback_alias, &e, &w, &m, &context)
        .is_ok());

    let reuse = source.replace(
        "let mask=streamFactory(seed^17);",
        "seed=streamFactory(seed^17);",
    );
    let (a, e, w, m) = lifecycle_fixture(&reuse);
    assert!(a
        .certify_stream_instance(&reuse, &e, &w, &m, &context)
        .is_ok());
    let alias = source.replace("created.init();", "let alias=created;alias.init();");
    let (algorithms, encoding, wiring, emission) = lifecycle_fixture(&alias);
    assert!(algorithms
        .certify_stream_instance(&alias, &encoding, &wiring, &emission, &context)
        .is_ok());
}
#[test]
fn lifecycle_mutations_are_refused_by_instance_api() {
    let source = lifecycle_source();
    let context = lifecycle::InitializationContext::from_initializer(
        "e.detail.init('synthetic-token');",
        "synthetic-token",
    )
    .unwrap();
    for (case, mutant) in [
        source.replace(";start();", ";tokenState^=1;start();"),
        source.replace(";start();", ";tokenState++;start();"),
        source.replace("created.init();", "created.init();created.init();"),
        source.replace("created.init();", "created.start();created.init();"),
        source.replace("created.init();", "unknown(created);created.init();"),
        source.replace(
            "created.init();",
            "let alias=created;unknown(alias);alias.init();",
        ),
        source.replace(
            "created.init();",
            "let alias=created;alias.start();alias.init();",
        ),
        source.replace(
            "let mask=streamFactory(seed^17);",
            "seed^=1;let mask=streamFactory(seed^17);",
        ),
        source.replace("onInit(function callback", "unknown(function callback"),
        source.replace(
            "created.init();",
            "let other=new SessionCtor(token);other.init();",
        ),
        source.replace(";start();", ";unknown(start);"),
        source.replace(
            "tokenState=hashAlias(context.transactionToken);start();",
            "start();tokenState=hashAlias(context.transactionToken);",
        ),
        source.replace("transactionToken:token", "transactionToken:'different'"),
        source.replace(
            "producer(null,context,function(cb){stored=cb;});stored();",
            "unknown(producer);producer(null,context,function(cb){stored=cb;});stored();",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let (algorithms, encoding, wiring, emission) = lifecycle_fixture(&mutant);
        assert!(
            algorithms
                .certify_stream_instance(&mutant, &encoding, &wiring, &emission, &context)
                .is_err(),
            "mutation {case}"
        );
    }
    assert!(lifecycle::InitializationContext::from_initializer(
        "e.detail.init('different');",
        "synthetic-token"
    )
    .is_err());
}

// Recover optional instance/prefix binding fields missing from old saved
// contracts by exact emitted-buffer and seed-symbol data flow, never by values.
fn bind_private_instance(
    source: &str,
    encoding: &EncodingProperties,
    emission: &mut crate::signals::extract::contracts::EmissionContract,
) {
    let arena = Allocator::default();
    let parsed = Parser::new(&arena, source, SourceType::mjs()).parse();
    let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
    let scopes = semantic.scoping();
    let nodes = semantic.nodes();
    let call = nodes
        .iter()
        .find_map(|n| match n.kind() {
            AstKind::CallExpression(c) if SourceSpan::from(c.span) == emission.site => Some(c),
            _ => None,
        })
        .unwrap();
    let Expression::ArrayExpression(payload) = call.arguments[0]
        .as_expression()
        .unwrap()
        .without_parentheses()
    else {
        panic!("payload array")
    };
    let Expression::Identifier(buffer) = payload.elements[1]
        .as_expression()
        .unwrap()
        .without_parentheses()
    else {
        panic!("buffer binding")
    };
    let buffer = buffer
        .reference_id
        .get()
        .and_then(|r| scopes.get_reference(r).symbol_id())
        .unwrap();
    let prefix=nodes.iter().find_map(|n|match n.kind(){AstKind::VariableDeclarator(d) if matches!(&d.id,BindingPattern::BindingIdentifier(b) if b.symbol_id.get()==Some(buffer))=>d.init.as_ref().and_then(|e|if let Expression::ArrayExpression(a)=e.without_parentheses(){Some(a)}else{None}),_=>None}).unwrap();
    let Expression::BinaryExpression(shift) = prefix.elements[0]
        .as_expression()
        .unwrap()
        .without_parentheses()
    else {
        panic!("prefix shift")
    };
    let Expression::Identifier(seed) = shift.left.without_parentheses() else {
        panic!("seed")
    };
    let seed = seed
        .reference_id
        .get()
        .and_then(|r| scopes.get_reference(r).symbol_id())
        .unwrap();
    let instances: Vec<_> = nodes
        .iter()
        .filter_map(|n| match n.kind() {
            AstKind::CallExpression(c)
                if function_origin(&c.callee, scopes, nodes, 0)
                    == encoding.byte_stream.as_ref().map(|s| s.factory.span) =>
            {
                let Expression::BinaryExpression(b) =
                    c.arguments.first()?.as_expression()?.without_parentheses()
                else {
                    return None;
                };
                let Expression::Identifier(id) = b.left.without_parentheses() else {
                    return None;
                };
                (id.reference_id
                    .get()
                    .and_then(|r| scopes.get_reference(r).symbol_id())
                    == Some(seed))
                .then_some(c.span)
            }
            _ => None,
        })
        .collect();
    assert_eq!(instances.len(), 1, "one concrete source seed instance");
    let stream = emission.stream.as_mut().unwrap();
    stream.instance = Some(instances[0].into());
    stream.seed_binding = Some(scopes.symbol_span(seed).into());
    fn update(node: &mut crate::signals::extract::contracts::GrammarNode, seed: SourceSpan) {
        use crate::signals::extract::contracts::GrammarNode;
        match node {
            GrammarNode::RawPrefix(p) => p.seed_binding = Some(seed),
            GrammarNode::Sequence { items } => {
                for n in items {
                    update(n, seed)
                }
            }
            _ => {}
        }
    }
    update(&mut emission.grammar, scopes.symbol_span(seed).into());
}

#[test]
fn callback_truthiness_guards_observe_without_exposing_the_value() {
    let context = lifecycle::InitializationContext::from_initializer(
        "e.detail.init('synthetic-token');",
        "synthetic-token",
    )
    .unwrap();
    for guard in [
        "if(stored)stored();",
        "stored && stored();",
        "if(!stored)return;stored();",
        "stored ? stored() : void 0;",
    ] {
        let source = lifecycle_source().replace("stored();", guard);
        let (a, e, w, m) = lifecycle_fixture(&source);
        assert!(
            a.certify_stream_instance(&source, &e, &w, &m, &context)
                .is_ok(),
            "{guard}"
        );
    }
    for escape in [
        "consume(stored);stored();",
        "let escaped=stored||null;stored();",
        "return stored&&stored;",
    ] {
        let source = lifecycle_source().replace("stored();", escape);
        let (a, e, w, m) = lifecycle_fixture(&source);
        assert!(
            a.certify_stream_instance(&source, &e, &w, &m, &context)
                .is_err(),
            "{escape}"
        );
    }
}

#[test]
fn named_registered_callbacks_keep_single_origin_and_consumers() {
    let context = lifecycle::InitializationContext::from_initializer(
        "e.detail.init('synthetic-token');",
        "synthetic-token",
    )
    .unwrap();
    let inline = lifecycle_source();
    let named = inline
        .replace("onInit(function callback(){", "function callback(){")
        .replace(
            "emitters.next([[],b]);\n        });",
            "emitters.next([[],b]);\n        } onInit(callback);",
        );
    for source in [
        named.clone(),
        named.replace("onInit(callback);", "let alias=callback;onInit(alias);"),
    ] {
        let (a, e, w, m) = lifecycle_fixture(&source);
        assert!(a
            .certify_stream_instance(&source, &e, &w, &m, &context)
            .is_ok());
    }
    for replacement in [
        "onInit(callback);consume(callback);",
        "onInit(callback);onInit(callback);",
        "callback=other;onInit(callback);",
    ] {
        let source = named.replace("onInit(callback);", replacement);
        let (a, e, w, m) = lifecycle_fixture(&source);
        assert!(a
            .certify_stream_instance(&source, &e, &w, &m, &context)
            .is_err());
    }
}

#[test]
fn nested_emission_helpers_require_initialized_creation_or_every_caller() {
    let context = lifecycle::InitializationContext::from_initializer(
        "e.detail.init('synthetic-token');",
        "synthetic-token",
    )
    .unwrap();
    let source = lifecycle_source()
        .replace(
            "onInit(function callback(){",
            "onInit(function callback(){function produce(){",
        )
        .replace(
            "emitters.next([[],b]);\n        });",
            "emitters.next([[],b]);\n        } produce(); });",
        );
    let (a, e, w, m) = lifecycle_fixture(&source);
    assert!(a
        .certify_stream_instance(&source, &e, &w, &m, &context)
        .is_ok());
    // This declaration is instantiated only when the proven initialized
    // callback enters, so even an opaque consumer cannot invoke it earlier.
    let changed = source.replace("produce();", "consume(produce);produce();");
    let (a, e, w, m) = lifecycle_fixture(&changed);
    assert!(a
        .certify_stream_instance(&changed, &e, &w, &m, &context)
        .is_ok());
    for replacement in ["produce=unknown;produce();"] {
        let changed = source.replace("produce();", replacement);
        let (a, e, w, m) = lifecycle_fixture(&changed);
        assert!(a
            .certify_stream_instance(&changed, &e, &w, &m, &context)
            .is_err());
    }
    // A producer-level declaration lacks that initialized-entry witness.
    let changed = source
        .replace(
            "onInit(function callback(){function produce(){",
            "function produce(){",
        )
        .replace("} produce(); });", "} consume(produce);");
    let (a, e, w, m) = lifecycle_fixture(&changed);
    assert!(a
        .certify_stream_instance(&changed, &e, &w, &m, &context)
        .is_err());
}

#[test]
fn inline_closure_creation_cannot_precede_its_initialized_parent_activation() {
    let context = lifecycle::InitializationContext::from_initializer(
        "e.detail.init('synthetic-token');",
        "synthetic-token",
    )
    .unwrap();
    let source = lifecycle_source()
        .replace(
            "onInit(function callback(){",
            "onInit(function callback(){schedule('event',function later(){",
        )
        .replace(
            "emitters.next([[],b]);\n        });",
            "emitters.next([[],b]);\n        }); });",
        );
    let (a, e, w, m) = lifecycle_fixture(&source);
    assert!(a
        .certify_stream_instance(&source, &e, &w, &m, &context)
        .is_ok());
    let changed = source
        .replace(
            "onInit(function callback(){schedule('event',function later(){",
            "schedule('event',function later(){",
        )
        .replace("}); });", "});");
    let (a, e, w, m) = lifecycle_fixture(&changed);
    assert!(a
        .certify_stream_instance(&changed, &e, &w, &m, &context)
        .is_err());
}

#[test]
fn prefix_and_stream_can_read_the_same_unchanged_seed_in_either_order() {
    let context = lifecycle::InitializationContext::from_initializer(
        "e.detail.init('synthetic-token');",
        "synthetic-token",
    )
    .unwrap();
    let source=lifecycle_source().replace("let b=[seed>>>24,seed>>16&255,seed>>8&255,seed&255];\n            let mask=streamFactory(seed^17);", "let mask=streamFactory(seed^17);\n            let b=[seed>>>24,seed>>16&255,seed>>8&255,seed&255];");
    let (a, e, w, m) = lifecycle_fixture(&source);
    assert!(a
        .certify_stream_instance(&source, &e, &w, &m, &context)
        .is_ok());
    for changed in [
        source.replace(
            "let mask=streamFactory(seed^17);",
            "let mask=streamFactory(seed^17);++seed;",
        ),
        source.replace(
            "let mask=streamFactory(seed^17);",
            "let mask=(seed=streamFactory(seed^17));",
        ),
    ] {
        let (a, e, w, m) = lifecycle_fixture(&changed);
        assert!(a
            .certify_stream_instance(&changed, &e, &w, &m, &context)
            .is_err());
    }
}

#[test]
fn nullish_callback_guards_do_not_invoke_or_escape_the_callback() {
    let context = lifecycle::InitializationContext::from_initializer(
        "e.detail.init('synthetic-token');",
        "synthetic-token",
    )
    .unwrap();
    for guard in [
        "if(stored!==void 0)stored();",
        "if(stored!=undefined)stored();",
        "if(stored===null)return;stored();",
    ] {
        let source = lifecycle_source().replace("stored();", guard);
        let (a, e, w, m) = lifecycle_fixture(&source);
        assert!(a
            .certify_stream_instance(&source, &e, &w, &m, &context)
            .is_ok());
    }
    let source = lifecycle_source().replace("stored();", "if(stored!==void effect())stored();");
    let (a, e, w, m) = lifecycle_fixture(&source);
    assert!(a
        .certify_stream_instance(&source, &e, &w, &m, &context)
        .is_err());
}

#[test]
fn immutable_stored_callback_snapshot_follows_registration() {
    let context = lifecycle::InitializationContext::from_initializer(
        "e.detail.init('synthetic-token');",
        "synthetic-token",
    )
    .unwrap();
    let source = lifecycle_source().replace("stored();", "let run=stored;if(run!==void 0)run();");
    let (a, e, w, m) = lifecycle_fixture(&source);
    assert!(a
        .certify_stream_instance(&source, &e, &w, &m, &context)
        .is_ok());
    for replacement in [
        "let run=stored;run=other;run();",
        "let run=stored;consume(run);",
        "stored=other;let run=stored;run();",
    ] {
        let source = lifecycle_source().replace("stored();", replacement);
        let (a, e, w, m) = lifecycle_fixture(&source);
        assert!(a
            .certify_stream_instance(&source, &e, &w, &m, &context)
            .is_err());
    }
    let source = lifecycle_source()
        .replace(
            "let stored;producer(",
            "let stored;let run=stored;producer(",
        )
        .replace("stored();", "run();");
    let (a, e, w, m) = lifecycle_fixture(&source);
    assert!(a
        .certify_stream_instance(&source, &e, &w, &m, &context)
        .is_err());
}
