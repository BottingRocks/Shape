use super::*;

fn number_call(source: &str) -> crate::signals::extract::contracts::WriteNode {
    let arena = Allocator::default();
    let parsed = Parser::new(&arena, source, SourceType::mjs()).parse();
    let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
    let call = semantic
        .nodes()
        .iter()
        .find_map(|n| match n.kind() {
            AstKind::CallExpression(c)
                if matches!(c.arguments.get(1).and_then(|a| a.as_expression()),
                Some(Expression::NumericLiteral(n)) if n.value == 9.) =>
            {
                Some(c)
            }
            _ => None,
        })
        .unwrap();
    serde_json::from_value(serde_json::json!({
        "span": [call.span.start, call.span.end],
        "value_span": [call.arguments[1].span().start, call.arguments[1].span().end],
        "value": "9", "resolved": null, "mask": "stream", "mask_source": "mask",
        "position": "position", "xor_literal": null
    }))
    .unwrap()
}

#[test]
fn number_factory_is_the_value_at_the_call_before_cursor_reuse() {
    let writer = include_str!("reference/number.js").replace("local0(", "integerWriter(");
    let source = format!(
        "{writer}\nfunction collect(mask, buffer, position) {{
        let callable; callable = integerWriter;
        callable = callable(mask, 9, buffer, position);
        callable = 0;
    }}"
    );
    let write = number_call(&source);
    let result = number_at_call(&source, &write);
    assert_eq!(result.codec.as_deref(), Some("sign-magnitude-5-7/1"));
    assert_eq!(result.span.unwrap().start, 0);
    for changed in [
        source.replace(
            "callable = callable(mask",
            "callable = foreign; callable = callable(mask",
        ),
        source.replace(
            "callable = integerWriter;",
            "if (position) callable = integerWriter;",
        ),
        source.replace(
            "callable = integerWriter;",
            "callable = integerWriter; function mutate(){callable = foreign;} mutate();",
        ),
        source.replace("< 4096", "< 4095"),
        source.replace("function collect(mask", "function collect(Number, mask"),
    ] {
        // Shadowing collect's Number does not shadow the outer writer.
        let expected = changed.contains("function collect(Number,");
        assert_eq!(
            number_at_call(&changed, &number_call(&changed))
                .codec
                .is_some(),
            expected
        );
    }
    let shadowed = format!("function scope(Number){{{source}}}");
    assert!(number_at_call(&shadowed, &number_call(&shadowed))
        .codec
        .is_none());
    let captured = source
        .replace(
            "function collect(",
            "let shared; shared=integerWriter; function collect(",
        )
        .replace("callable = integerWriter;", "callable = shared;");
    assert!(number_at_call(&captured, &number_call(&captured))
        .codec
        .is_some());
    for changed in [
        captured.replace(
            "shared=integerWriter;",
            "shared=integerWriter; shared=foreign;",
        ),
        captured.replace(
            "shared=integerWriter;",
            "if (condition) shared=integerWriter;",
        ),
        captured.replace("shared=integerWriter;", "shared=integerWriter; eval(code);"),
        source.replace("callable = integerWriter;", ""),
        source
            .replace("let callable; callable = integerWriter;", "")
            .replace("callable = 0;", "let callable = integerWriter;"),
        source.replace(
            "function integerWriter(local1, local2",
            "function integerWriter(local2, local1",
        ),
    ] {
        assert!(number_at_call(&changed, &number_call(&changed))
            .codec
            .is_none());
    }
    let mut forged = write;
    forged.value_span.end -= 1;
    assert!(number_at_call(&source, &forged).codec.is_none());
}

#[test]
fn whole_body_proofs_reject_changes_to_recurrence_warmup_and_hash() {
    let span = SourceSpan::new(0, 1);
    for reference in [
        include_str!("reference/stream.js"),
        include_str!("reference/hash.js"),
    ] {
        assert!(proof(Some((span, reference)), reference, "test", false)
            .codec
            .is_some());
    }
    let stream = include_str!("reference/stream.js");
    for changed in [
        stream.replacen("26", "25", 1),
        stream.replacen("local6();", "", 1),
        stream.replacen("& 255", "& 254", 1),
    ] {
        assert!(proof(Some((span, &changed)), stream, "test", false)
            .codec
            .is_none());
    }
    let hash = include_str!("reference/hash.js");
    let changed = hash.replacen("<< 5", "<< 6", 1);
    assert!(proof(Some((span, &changed)), hash, "test", false)
        .codec
        .is_none());
}
#[test]
fn integer_certification_is_separate_from_float_compatibility() {
    let number = include_str!("reference/number.js");
    let span = SourceSpan::new(0, 1);
    assert!(proof(Some((span, number)), number, "test", true)
        .codec
        .is_some());
    let changed = number.replacen("< 4096", "< 4095", 1);
    assert!(proof(Some((span, &changed)), number, "test", true)
        .codec
        .is_none());
    let float_changed = number.replacen("Math.log", "Math.sqrt", 1);
    assert!(proof(Some((span, &float_changed)), number, "test", true)
        .codec
        .is_some());
    assert!(proof(Some((span, &float_changed)), number, "test", false)
        .codec
        .is_none());
}
#[test]
fn alpha_equivalence_keeps_binding_and_parameter_identity() {
    assert_eq!(
        canonical("function f(a){let b;b=a;return b;}", false),
        canonical("function g(x){let y=x;return y;}", false)
    );
    assert_ne!(
        canonical("function f(a,b){return a-b;}", false),
        canonical("function f(a,b){return b-a;}", false)
    );
    assert_ne!(
        canonical("function f(a){return external(a);}", false),
        canonical("function f(a){let external=a;return external(a);}", false)
    );
}

/// Only recognizer metadata is supplied here. Every executable dependency is
/// resolved from this synthetic source by the real certifier.
pub(super) fn fixture_inputs(source: &str) -> (EncodingProperties, WiringProperties) {
    use serde_json::json;
    let arena = Allocator::default();
    let parsed = Parser::new(&arena, source, SourceType::mjs()).parse();
    assert!(parsed.errors.is_empty());
    let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
    let function = |name: &str| {
        semantic
            .nodes()
            .iter()
            .find_map(|n| match n.kind() {
                AstKind::Function(f) if f.id.as_ref().is_some_and(|i| i.name == name) => {
                    Some(json!({"span": SourceSpan::from(f.span), "binding": null,
                    "body": null, "name_label": null, "source": ""}))
                }
                _ => None,
            })
            .unwrap()
    };
    let state = semantic
        .scoping()
        .symbol_ids()
        .find(|s| semantic.scoping().symbol_name(*s) == "tokenState")
        .unwrap();
    let state_span: SourceSpan = semantic.scoping().symbol_span(state).into();
    let construction = semantic.nodes().iter().find_map(|n| match n.kind() {
        AstKind::NewExpression(c) if matches!(&c.callee, Expression::Identifier(id) if id.name == "session") => Some(SourceSpan::from(c.span)),
        _ => None,
    });
    let public_entry = if construction.is_some() {
        function("entry")
    } else {
        function("session")
    };
    let identity = json!({"sha256": digest(source.as_bytes()), "bytes": source.len()});
    let encoding: EncodingProperties = serde_json::from_value(json!({
        "schema_version": 2, "source": identity,
        "seed_link": {"hash": function("tokenHash"), "seed_reads": [], "runtime_value": true, "unresolved": []},
        "number": {"function": function("integerWriter"), "parameters": [], "branches": [],
            "buffer_write_count": 0, "continuation_masks": [], "shifts": [], "float_scratch": null, "fallback": null},
        "string": {"factory": function("stringFactory"), "writer": null, "parameters": [],
            "cache": null, "counter": null, "miss_traversal": [], "terminator": null,
            "terminator_is_literal_zero": true, "number_dependency": null, "unresolved": []},
        "byte_stream": {"factory": function("streamFactory"), "cycle": null, "captured_state": [],
            "warmup_calls": null, "cycle_modulus": null, "word_updates": [], "extraction_shifts": [],
            "seed_parameter": null, "local_seed": null, "per_caller_prefix": false,
            "shared_cursor_writeback": null, "unresolved": []}, "unresolved": []
    })).unwrap();
    let wiring: WiringProperties = serde_json::from_value(json!({
        "schema_version": 1, "source": identity, "main": function("session"), "main_argument": null,
        "descriptors": null, "groups": [], "direct_routes": [], "coordinator": null,
        "stores": {"observer_factory": null, "latest_cells": null, "status_cells": null,
            "notifier": null, "snapshot": null, "prefix_scratch": null, "evidence": [], "unresolved": []},
        "public_init": {"function": public_entry, "constructor_call_span": construction.unwrap_or(state_span),
            "arguments": [], "token": {"transaction_token_field": "transactionToken",
                "state_binding": {"symbol": state.index(), "name": "tokenState", "declaration": state_span,
                    "identifier": state_span, "owned_by_main": true},
                "hash_function": function("tokenHash"), "input_source": "context.transactionToken",
                "constructor_argument": 0, "public_init_argument": 0, "evidence": [], "unresolved": []}},
        "collector_requests": [], "unresolved": [], "diagnostics": []
    })).unwrap();
    (encoding, wiring)
}
fn fixture(source: &str) -> AlgorithmReport {
    let (encoding, wiring) = fixture_inputs(source);
    certify(source, &encoding, &wiring)
}

pub(super) fn session(reference: &str) -> String {
    let stream = include_str!("reference/stream.js")
        .replace("local0(", "streamFactory(")
        .replace("local9", "tokenState");
    let number = include_str!("reference/number.js").replace("local0(", "integerWriter(");
    let string = reference
        .replace("local0(", "stringFactory(")
        .replace("local10(", "writeIndex(")
        .replace("local15(", "readUnit(");
    let hash = include_str!("reference/hash.js")
        .replace("local0(", "tokenHash(")
        .replace("local4(", "hashUnit(");
    format!(
        "function session(token){{let context={{transactionToken:void 0}};context.transactionToken=token;
        let tokenState;let local5=true;
        function unit(s){{return s.charCodeAt();}}
        let readUnit=unit;let hashUnit=unit;let writeIndex=integerWriter;let hashAlias=tokenHash;
        {stream}\n{number}\n{string}\n{hash}
        tokenState=hashAlias(context.transactionToken);
        function collect(seed){{return streamFactory(seed);}}
        return collect(7);
    }}
    function entry(token){{return new session(token);}}"
    )
}

#[test]
fn certification_accepts_generic_references_and_single_origin_aliases() {
    for reference in [
        include_str!("reference/string.js"),
        include_str!("reference/string-main.js"),
        include_str!("reference/string-direct-nul-masked-tail.js"),
    ] {
        let source = session(reference);
        let report = fixture(&source);
        assert!(
            report.hash.codec.is_some(),
            "hash: {:?}",
            report.hash.reason
        );
        assert!(
            report.stream.codec.is_some(),
            "stream: {:?}",
            report.stream.reason
        );
        assert!(
            report.integer.codec.is_some(),
            "integer: {:?}",
            report.integer.reason
        );
        assert!(
            report.string.codec.is_some(),
            "string: {:?}",
            report.string.reason
        );
        assert!(report.float.codec.is_none());
        let factory_alias = source
            .replace(
                "let tokenState;",
                "let tokenState;let makeStream=streamFactory;",
            )
            .replace("return streamFactory(seed);", "return makeStream(seed);");
        assert!(fixture(&factory_alias).stream.codec.is_some());
        let assigned_hash_alias = source.replace(
            "let hashAlias=tokenHash;",
            "let hashAlias;hashAlias=tokenHash;",
        );
        assert!(fixture(&assigned_hash_alias).stream.codec.is_some());
        let bound = source.replace(
            "let readUnit=unit",
            "let readUnit=Array.call.bind(String.prototype.charCodeAt)",
        );
        assert!(fixture(&bound).string.codec.is_some());
    }
}

#[test]
fn direct_nul_masked_tail_is_exactly_the_reviewed_public_variant() {
    // Keep this reference derived from the committed generic body, with only
    // the two reviewed statement changes. No captured source is a fixture.
    let original = include_str!("reference/string.js");
    let nul =
        "++local13;\n            local14 = 128;\n            local7[local13] = local2() ^ 128;";
    let tail = "local14 = local14 & 63 | 128;\n              local7[local13] = local14 ^ local2();";
    assert_eq!(original.matches(nul).count(), 1);
    assert_eq!(original.matches(tail).count(), 1);
    let reviewed = original
        .replace(nul, "local7[++local13] = local2() ^ 128;")
        .replace(
            tail,
            "local14 = (local14 & 63 | 128) ^ local2();\n              local7[local13] = local14;",
        );
    assert_eq!(
        reviewed,
        include_str!("reference/string-direct-nul-masked-tail.js")
    );
    let renamed = reviewed
        .replace("local14", "unitValue")
        .replace("local13", "cursor")
        .replace("local7", "buffer");
    for reference in [&reviewed, &renamed] {
        let source = format!("/* π 🧪: byte offsets */\n{}", session(reference));
        let report = fixture(&source);
        assert_eq!(
            report.string.codec.as_deref(),
            Some("modified-utf8-intern-129/1")
        );
        let span = report.string.span.unwrap();
        let body = &source[span.start as usize..span.end as usize];
        assert_eq!(report.string.sha256, Some(digest(body.as_bytes())));
        // Existing references must not silently grow algebraic equivalence.
        for old in [original, include_str!("reference/string-main.js")] {
            assert!(proof(Some((span, body)), old, "test", false)
                .codec
                .is_none());
        }
    }
}

#[test]
fn direct_nul_masked_tail_rejects_byte_draw_cache_and_reset_mutations() {
    let reference = include_str!("reference/string-direct-nul-masked-tail.js");
    for (from, to) in [
        ("local2() ^ 128;", "local2() ^ 130;"),
        ("local2() ^ 192;", "local2() ^ 193;"),
        ("^ local2();", "^ local2() ^ local2();"),
        (
            "local14 = (local14 & 63 | 128) ^ local2();",
            "local14 = (local14 & 63 | 128) | local2();",
        ),
        ("local2() ^ 129;", "local2() ^ 130;"),
        ("Object.create(null)", "{}"),
        ("Object.create(null)", "sharedCache"),
        ("local4 = local12 + 1;", "local4 = 0;"),
        ("let local9;", "let local9; local3 = Object.create(null);"),
    ] {
        assert!(reference.contains(from), "mutation must change the fixture");
        let source = session(&reference.replacen(from, to, 1));
        assert!(fixture(&source).string.codec.is_none(), "accepted {from}");
    }
}

#[test]
fn direct_nul_masked_tail_preserves_emission_resource_proofs() {
    use crate::signals::extract::{contracts::*, layout::*};
    use crate::signals::wire::program::{compile, Program, WireValue};

    fn compile_body(body: &str) -> Result<Program, crate::signals::wire::WireError> {
        let reference = include_str!("reference/string-direct-nul-masked-tail.js");
        let collector = format!(
            "function collect(obs){{let seed=Math.random()*4294967296|0;
            let m=streamFactory(seed^17);let sw=stringFactory(m);
            let b=[seed>>>24,seed>>16&255,seed>>8&255,seed&255];
            let p=3;{body}obs.next([[],b]);}}"
        );
        let source = session(reference).replace(
            "function collect(seed){return streamFactory(seed);}",
            &collector,
        );
        assert!(source.contains(&collector));
        let (encoding, wiring) = fixture_inputs(&source);
        let algorithms = certify(&source, &encoding, &wiring);
        assert!(
            algorithms.stream.codec.is_some(),
            "{:?}",
            algorithms.stream.reason
        );
        assert!(
            algorithms.string.codec.is_some(),
            "{:?}",
            algorithms.string.reason
        );
        let start = source.find("function collect(").unwrap();
        let report = extract_signal_layouts(&SignalLayoutRequest {
            source: &source,
            helpers: encoding.helper_bindings(),
            collectors: vec![CollectorRequest {
                function: oxc_span::Span::new(start as u32, (start + collector.len()) as u32),
                observers: ObserverWiring::Direct { signal: 23 },
                site: EmissionSite::ObserverNext,
                forwarded_parameter: None,
            }],
            limits: SignalLayoutLimits::default(),
        })
        .unwrap();
        let emission = &report.signals[0].emissions[0];
        let contract = EmissionContract {
            site: emission.origin.into(),
            forwarding_sites: vec![],
            quote: String::new(),
            route: emission.route.clone(),
            phase: None,
            payload: emission.variant.clone(),
            stream: emission.mask.as_ref().map(|mask| StreamContract {
                argument: mask.argument.clone(),
                seed_word: mask.seed_word.clone(),
                salt: mask.xor_source.clone(),
                salt_span: Some(mask.xor_span.into()),
                instance: mask.instance,
                seed_binding: mask.seed_binding,
            }),
            rendered: emission.grammar.render(),
            grammar: emission.grammar.clone(),
        };
        compile(&contract, &algorithms, &source)
    }

    let program = compile_body("p=sw(first,b,p);p=sw(second,b,p);").unwrap();
    let units = vec![0, 65, 0xd83d, 0xde00, 0xd800];
    let value = WireValue::Sequence(vec![
        WireValue::StringUnits(units.clone()),
        WireValue::StringUnits(units),
    ]);
    let bytes = program.encode(&value, 7, 11).unwrap();
    assert_eq!(program.decode(&bytes, 11).unwrap(), value);
    // A fresh message starts its own table, with no prior intern history.
    assert_eq!(program.encode(&value, 7, 11).unwrap(), bytes);
    for body in [
        "consume(sw);p=sw(first,b,p);",
        "p=sw(first,b,p);sw=stringFactory(m);p=sw(second,b,p);",
        "m();p=sw(first,b,p);",
        "sw=foreignWriter;p=sw(first,b,p);",
    ] {
        assert!(compile_body(body).is_err(), "accepted {body}");
    }
}

#[test]
fn declaration_mutants_preserve_mutability_scope_and_diagnostics() {
    let reference = include_str!("reference/string.js");
    let source = session(reference);
    assert!(
        fixture(&source.replace("let local4 = 0;", "const local4 = 0;"))
            .string
            .codec
            .is_none()
    );
    assert!(
        fixture(&source.replace("let local4 = 0;", "var local4 = 0;"))
            .string
            .codec
            .is_some()
    );
    assert!(
        fixture(&source.replace("let local4 = 0;", "{let local4 = 0;}"))
            .string
            .codec
            .is_none()
    );
    assert!(
        fixture(&source.replace("let local4 = 0;", "let local4;local4 = 0;"))
            .string
            .codec
            .is_some()
    );
    assert!(canonical("function f(){let a=0;let a=1;return a;}", false).is_none());
    assert!(canonical("function f(){let a=a;return a;}", false).is_none());
    assert!(canonical(
        "function f(){let a=g();let b=0;function g(){return b;}return a;}",
        false
    )
    .is_none());
    assert!(canonical("function f(){let a=obj.value;let b=0;return b;}", false).is_none());
    assert!(canonical("function f(x){let a=x+0;let b=0;return b;}", false).is_none());
    assert!(canonical("function f(){a=1;let a;return a;}", false).is_none());
    assert!(canonical("function f(){a=1;var a;return a;}", false).is_none());
    assert_ne!(
        canonical("function f(){let a;return a;}", false),
        canonical("function f(){{let a;}return a;}", false)
    );
}

#[test]
fn dependency_calls_are_bound_to_their_reference_roles() {
    for reference in [
        include_str!("reference/string.js"),
        include_str!("reference/string-main.js"),
        include_str!("reference/string-direct-nul-masked-tail.js"),
    ] {
        let source = session(reference);
        for mutant in [
            source.replace("let readUnit=unit", "let readUnit=integerWriter"),
            source.replace("let writeIndex=integerWriter", "let writeIndex=unit"),
            source
                .replace("let readUnit=unit", "let readUnit=integerWriter")
                .replace("let writeIndex=integerWriter", "let writeIndex=unit"),
            source.replace("return s.charCodeAt();", "return s.charCodeAt()+1;"),
            source.replace("let readUnit=unit", "let readUnit=unknownUnit"),
            source.replace(
                "readUnit(local6[local12], 0)",
                "readUnit(local6[local12], 1)",
            ),
        ] {
            assert!(fixture(&mutant).string.codec.is_none());
        }
    }
    let source = session(include_str!("reference/string.js"));
    for mutant in [
        source.replace("let hashUnit=unit", "let hashUnit=integerWriter"),
        source.replace(
            "typeof local1 === \"string\"",
            "typeof local1 === \"number\"",
        ),
        source.replace("let hashUnit=unit", "let hashUnit=unknownUnit"),
        source
            .replace(
                "typeof local1 === \"string\"",
                "classify(local1) === \"string\"",
            )
            .replace(
                "let hashUnit=unit",
                "let hashUnit=unit;function classify(x){return \"string\";}",
            ),
        source.replace("return s.charCodeAt();", "return s.charCodeAt()+1;"),
    ] {
        let report = fixture(&mutant);
        assert!(report.hash.codec.is_none());
        assert!(report.stream.codec.is_none());
    }
}

#[test]
fn stream_requires_unmodified_hash_value_and_initialization_before_escape() {
    let source = session(include_str!("reference/string.js"));
    for change in [
        "tokenState^=1;",
        "tokenState++;",
        "tokenState=0;",
        "function change(){tokenState^=1;}change();",
        "eval('tokenState=0');",
    ] {
        let mutant = source.replace(
            "tokenState=hashAlias(context.transactionToken);",
            &format!("tokenState=hashAlias(context.transactionToken);{change}"),
        );
        assert!(fixture(&mutant).stream.codec.is_none(), "{change}");
    }
    let early = source.replace(
        "tokenState=hashAlias(context.transactionToken);",
        "collect(1);tokenState=hashAlias(context.transactionToken);",
    );
    assert!(fixture(&early).stream.codec.is_none());
    let overwritten = source.replace(
        "tokenState=hashAlias(context.transactionToken);",
        "hashAlias=function(x){return 0;};tokenState=hashAlias(context.transactionToken);",
    );
    assert!(fixture(&overwritten).stream.codec.is_none());
    let different = source.replace(
        "let hashAlias=tokenHash;",
        "let hashAlias=otherHash;function otherHash(x){return x.length;}",
    );
    assert!(fixture(&different).stream.codec.is_none());
    let overwritten_twice = source.replace("tokenState=hashAlias(context.transactionToken);",
        "hashAlias=function(x){return 0;};hashAlias=tokenHash;tokenState=hashAlias(context.transactionToken);");
    assert!(fixture(&overwritten_twice).stream.codec.is_none());
    let changed_input = source.replace(
        "hashAlias(context.transactionToken)",
        "hashAlias(context.alternateToken)",
    );
    assert!(fixture(&changed_input).stream.codec.is_none());
    let conditional = source.replace(
        "tokenState=hashAlias(context.transactionToken);",
        "if(context.ready){tokenState=hashAlias(context.transactionToken);}",
    );
    assert!(fixture(&conditional).stream.codec.is_none());
    let initialized = source.replace("let tokenState;", "").replace(
        "tokenState=hashAlias(context.transactionToken);",
        "let tokenState=hashAlias(context.transactionToken);",
    );
    assert!(fixture(&initialized).stream.codec.is_some());
}

/// Optional static audit of a caller-supplied private source. No source,
/// evidence text, runtime values, or identifiers are emitted.
#[test]
#[ignore = "requires ALGORITHM_CERTIFICATION_SOURCE; static parsing only"]
fn private_source_certification_census() {
    let path = std::env::var_os("ALGORITHM_CERTIFICATION_SOURCE").expect("private source path");
    let source = std::fs::read_to_string(path).expect("read private source");
    let encoding = crate::signals::extract::encoding::extract_encoding_properties(&source)
        .unwrap_or_else(|_| panic!("encoding extraction failed"));
    let wiring = crate::signals::extract::wiring::extract_program_wiring(&source)
        .unwrap_or_else(|_| panic!("wiring extraction failed"));
    let report = certify(&source, &encoding, &wiring);
    let identities = [
        report.hash.codec.is_some(),
        report.stream.codec.is_some(),
        report.integer.codec.is_some(),
        report.string.codec.is_some(),
    ];
    println!(
        "certified hash/stream/integer/string: {identities:?}; float: {}",
        report.float.codec.is_some()
    );
    if report.stream.codec.is_none() {
        println!("static stream proof: {:?}", report.stream.reason);
    }
    // Generic regressions specify acceptance; this optional check is a census.
}
