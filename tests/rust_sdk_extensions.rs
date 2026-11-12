//! Public SDK extension checks. No network or generated captures.
use serde::Deserialize;
use serde_json::json;
use shape::sdk::{self, headers, payload, seeds, PackValue, SerializerOptions};

fn profile() -> headers::Profile {
    serde_json::from_str(include_str!("fixtures/sdk/generator/profile.json")).unwrap()
}

#[test]
fn pack_source_answers_and_validation_use_sdk_errors() {
    #[derive(Deserialize)]
    struct Answer {
        request: shape::signals::payload::superpack::PackRequest,
        hex: String,
        decodable: bool,
    }
    let answers: Vec<Answer> = serde_json::from_str(include_str!(
        "fixtures/generator/superpack-known-answers.json"
    ))
    .unwrap();
    for answer in answers {
        let bytes = sdk::pack(&answer.request.value, &answer.request.options).unwrap();
        let expected: Vec<u8> = answer
            .hex
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect();
        assert_eq!(bytes, expected);
        if answer.decodable {
            let decoded = sdk::unpack(&bytes, &answer.request.options).unwrap();
            assert_eq!(decoded.original_bytes, expected);
            assert!(decoded.canonical);
            assert_eq!(
                sdk::unpack_canonical(&bytes, &answer.request.options).unwrap(),
                decoded.value
            );
            assert_eq!(
                sdk::pack(&decoded.value, &answer.request.options).unwrap(),
                expected
            );
        } else {
            assert!(sdk::unpack(&bytes, &answer.request.options).is_err());
        }
    }
    let options = SerializerOptions::default();
    assert_eq!(sdk::unpack(&[], &options).unwrap_err().code, "failed");
    let duplicate = PackValue::Object(vec![
        ("x".into(), PackValue::Null),
        ("x".into(), PackValue::Null),
    ]);
    assert_eq!(sdk::pack(&duplicate, &options).unwrap_err().code, "failed");
}

#[test]
fn report_header_and_bare_payload_roundtrips_preserve_exact_bytes() {
    let mut p = profile();
    p.transaction_token = Some("public-test-token".into());
    p.chunk_size = Some(7);
    let bare = payload::codec(p.clone().into()).unwrap();
    let framed = headers::codec(p.clone()).unwrap();
    let report = payload::Report {
        errors: vec![PackValue::String("test".into())],
        positional_signals: vec![None, Some((vec![PackValue::Bool(true)], vec![0, 255]))],
        signals: vec![PackValue::from(42.0)],
        keyed_signals: vec![(7, Some((vec![PackValue::Null], vec![128])))],
        seeds: [u32::MAX, 123],
        keysets: vec![],
    };
    let context = headers::HeaderContext {
        b: true,
        f: true,
        c: Some("build".into()),
        d: None,
        z: Some("opaque".into()),
    };
    let mut document: headers::HeaderReport = report.clone().into();
    document.header_context = Some(context.clone());
    let encoded = payload::encode(&bare, &report).unwrap();
    assert_eq!(payload::decode(&bare, &encoded, 2, vec![]).unwrap(), report);
    let bundle = headers::encode(&framed, &document).unwrap();
    let parsed = headers::parse(&bundle.to_text()).unwrap();
    assert_eq!(
        headers::decode(&framed, &parsed, 2, vec![]).unwrap(),
        document
    );
    assert_eq!(
        headers::decode_finalized(&framed, &parsed).unwrap(),
        (encoded.clone(), context.clone())
    );
    assert_eq!(
        headers::encode_finalized(&framed, &encoded, Some(&context)).unwrap(),
        bundle
    );
    let decoded = headers::decode(&framed, &bundle, 2, vec![]).unwrap();
    assert_eq!(headers::encode(&framed, &decoded).unwrap(), bundle);

    let mut raw = payload::Payload::raw([9, 10], vec![0, 1, 128, 255]);
    raw.header_context = Some(context);
    let raw_headers = headers::encode_bytes(&framed, &raw).unwrap();
    assert_eq!(headers::decode_bytes(&framed, &raw_headers).unwrap(), raw);
    let bytes = raw.plaintext().unwrap();
    let bare_text = payload::encode_bytes(&bare, &bytes, raw.seeds).unwrap();
    let bare_decoded = payload::decode_bytes(&bare, &bare_text).unwrap();
    assert_eq!(bare_decoded.seeds, raw.seeds);
    assert_eq!(bare_decoded.plaintext().unwrap(), bytes);
    assert!(headers::parse("HTTP/1.1 200 OK\r\nx-a: a\r\n").is_err());
    assert!(payload::decode_bytes(&bare, "!").is_err());

    let mut wrong = p.clone();
    wrong.transaction_token = Some("wrong-token".into());
    let wrong = headers::codec(wrong).unwrap();
    assert_eq!(
        headers::decode_bytes(&wrong, &raw_headers)
            .unwrap_err()
            .code,
        "failed"
    );
    p.format = "invented-image/1".into();
    assert!(headers::codec(p.clone()).is_err());
    assert!(payload::codec(p.into()).is_err());
}

#[test]
fn byte_payload_uses_existing_public_known_answers() {
    #[derive(Deserialize)]
    struct Answer {
        profile: headers::Profile,
        seeds: [u32; 2],
        plaintext_hex: String,
        value: String,
    }
    let answers: Vec<Answer> =
        serde_json::from_str(include_str!("fixtures/generator/known-answers.json")).unwrap();
    for answer in answers {
        let bytes: Vec<u8> = answer
            .plaintext_hex
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect();
        let codec = payload::codec(answer.profile.into()).unwrap();
        assert_eq!(
            payload::encode_bytes(&codec, &bytes, answer.seeds).unwrap(),
            answer.value
        );
        let decoded = payload::decode_bytes(&codec, &answer.value).unwrap();
        assert_eq!(decoded.seeds, answer.seeds);
        assert_eq!(decoded.plaintext().unwrap(), bytes);
    }
}

#[test]
fn naming_corpus_tracks_source_identity_and_role_loss() {
    use sdk::naming::corpus::{self, Source};
    let before = "(function main(){ var registry=[1,2,3]; function entry(){return registry[0];} return {init:entry}; })();";
    let after = "(function main(){ var registry=[1,2,3]; return registry[0]; })();";
    let old = corpus::analyze(&[Source {
        name: "capture".into(),
        source: before.into(),
    }])
    .unwrap();
    let new = corpus::analyze(&[Source {
        name: "capture".into(),
        source: after.into(),
    }])
    .unwrap();
    assert_ne!(old[0].plan.source, new[0].plan.source);
    assert_eq!(old[0].plan, old[0].rename.plan);
    assert!(old[0].plan.recognized_roles().contains("session_bootstrap"));
    let differences = corpus::compare(&old, &new).unwrap();
    assert!(differences[0]
        .missing_roles
        .contains(&"session_bootstrap".into()));
    let same = corpus::compare(&old, &old).unwrap();
    assert!(same[0].missing_roles.is_empty());
    assert!(same[0].added_roles.is_empty());
    assert!(same[0].name_divergences.is_empty());
    let removed = corpus::compare(&old, &[]).unwrap();
    assert!(removed[0].baseline_present);
    assert!(!removed[0].current_present);
    let input = Source {
        name: "capture".into(),
        source: before.into(),
    };
    assert_eq!(
        corpus::analyze(&[input.clone(), input]).unwrap_err().code,
        "invalid_request"
    );
    assert!(corpus::analyze(&[Source {
        name: "bad".into(),
        source: "function {".into()
    }])
    .is_err());
}

#[test]
fn full_corpus_walk_uses_explicit_policy_and_core_convergence() {
    use sdk::trace::corpus::{self, CorpusOptions, KernelCachePolicy};
    let seed = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/javascript-vm-prefix.raw.js");
    let options = CorpusOptions {
        json: true,
        seeds: vec![seed.clone()],
        report_dir: Some(seed.clone()), // A file, deliberately not a cache directory.
        baseline: None,
        update: false,
        self_check: true,
        cleanup: true,
        jobs: 2,
        branches: None,
        audit: false,
        steps: 16,
        limit: None,
    };
    let actual = corpus::walk_seed(&seed, &options, KernelCachePolicy::Disabled).unwrap();
    assert!(!actual.is_empty());
    // Compare with the existing orchestration, not a second implementation.
    let expected = std::thread::Builder::new()
        .stack_size(shape::LARGE_STACK_BYTES)
        .spawn(move || {
            shape::trace::corpus::walk_seed_with_cache(
                &seed,
                &options,
                &KernelCachePolicy::Disabled,
            )
            .unwrap()
        })
        .unwrap()
        .join()
        .unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn trace_corpus_reports_regressions_and_rejects_wrong_shapes() {
    use sdk::trace::corpus::{self, ReachableOptions};
    let options = ReachableOptions::default();
    assert_eq!(
        (options.steps, options.jobs, options.limit, options.cleanup),
        (1_000_000, 1, None, false)
    );
    let baseline = json!({"public": {"0/0": {"complete": true, "positions": ["0/0"], "host": [], "creates": []}}});
    let current = json!({"public": {"0/0": {"complete": false, "stop": "step limit", "positions": ["0/0"], "host": [], "creates": []}}});
    let comparison = corpus::compare(&baseline, &current).unwrap();
    assert_eq!(comparison.regressions.len(), 1);
    assert!(comparison.regressions[0].contains("completed, now stops"));
    assert!(corpus::compare(&baseline, &baseline)
        .unwrap()
        .regressions
        .is_empty());
    assert_eq!(
        corpus::compare(&json!([]), &baseline).unwrap_err().code,
        "invalid_request"
    );
    assert_eq!(
        corpus::compare(&json!({"public": []}), &baseline)
            .unwrap_err()
            .code,
        "invalid_request"
    );
    assert!(corpus::compare_inline(&json!(null), &json!({})).is_err());
    // Options fail before source compilation or Directory cache IO.
    let directory =
        std::env::temp_dir().join(format!("shape-sdk-invalid-options-{}", std::process::id()));
    let options = ReachableOptions {
        jobs: 0,
        ..ReachableOptions::default()
    };
    let error = corpus::source(
        std::path::Path::new("public.js"),
        "function {",
        corpus::KernelCachePolicy::Directory(directory.clone()),
        options,
    )
    .err()
    .unwrap();
    assert_eq!(error.code, "invalid_request");
    assert!(!directory.exists());
}

#[test]
fn classification_preserves_public_domain_types_and_cross_vm_scope() {
    use sdk::ops::classify::{
        self, ClassifiedOp, OpNature, OpScope, SeedClassification, VmVariant,
    };
    let op = ClassifiedOp {
        handler_index: 7,
        kind: "property.get".into(),
        nature: OpNature::JavaScript,
        pops: 2,
        pushes: 1,
        source: "object[key]".into(),
    };
    let classified = classify::seeds(&[
        SeedClassification {
            variant: VmVariant::JavaScript,
            ops: vec![op.clone()],
        },
        SeedClassification {
            variant: VmVariant::WebAssembly,
            ops: vec![op],
        },
    ])
    .unwrap();
    assert_eq!(classified.len(), 1);
    assert_eq!(classified[0].scope, OpScope::Common);
    assert_eq!(classified[0].handlers, vec![(0, 7), (1, 7)]);
    assert!(classify::source("function {").is_err());

    let template = classify::HandlerTemplate {
        handler_index: 9,
        table_key: 9,
        semantic_identity: "synthetic-return".into(),
        source_span: shape::SpanRecord { start: 0, end: 9 },
        status: shape::TemplateStatus::Executable,
        steps: vec![shape::RecipeStep {
            order: 0,
            span: shape::SpanRecord { start: 0, end: 9 },
            operation: shape::RecipeOperation::Return {
                value: shape::SemanticValue::Literal(shape::LiteralValue::Number("1".into())),
            },
        }],
        unsupported: vec![],
    };
    let result = classify::template(&template, "return 1;").unwrap();
    assert_eq!(
        result,
        shape::classify_template(&template, "return 1;".into())
    );
    assert_eq!(result.source, "return 1;");
    assert_eq!(result.handler_index, 9);
}

struct MockHttp {
    panic: bool,
    calls: std::sync::Mutex<Vec<String>>,
}
impl seeds::HttpClient for MockHttp {
    fn get(
        &self,
        url: &seeds::Url,
        _: &seeds::Headers,
    ) -> shape::seeds::Result<seeds::HttpResponse> {
        assert!(!self.panic, "mock HTTP panic");
        self.calls.lock().unwrap().push(url.to_string());
        let body = if url.path() == "/launcher.js" {
            br#"var script = document.createElement('script'); script.src = '/kernel.js?seed=public_token'; document.head.appendChild(script);"#.to_vec()
        } else {
            include_bytes!("fixtures/kernels/javascript/kernel.raw.js").to_vec()
        };
        Ok(seeds::HttpResponse {
            status: 200,
            final_url: url.clone(),
            content_type: Some("application/javascript".into()),
            set_cookie: vec![],
            body,
        })
    }
}

#[test]
fn seeds_fetch_is_explicit_mockable_and_isolated() {
    let client = seeds::Arc::new(MockHttp {
        panic: false,
        calls: Default::default(),
    });
    let url = seeds::parse_url("https://example.invalid/launcher.js").unwrap();
    assert!(client.calls.lock().unwrap().is_empty());
    let fetched = seeds::fetch(client.clone(), &url, seeds::FetchOptions::default()).unwrap();
    assert_eq!(fetched.seed.as_str(), "public_token");
    assert_eq!(
        fetched.kernel.as_slice(),
        include_bytes!("fixtures/kernels/javascript/kernel.raw.js").as_slice()
    );
    assert_eq!(client.calls.lock().unwrap().len(), 2);
    assert_eq!(
        seeds::parse_kernel(&fetched.kernel).unwrap().sha256,
        fetched.kernel_summary.sha256
    );
    let direct =
        seeds::fetch_direct(client, &fetched.kernel_url, seeds::FetchOptions::default()).unwrap();
    // Direct acquisition intentionally strips the final configuration IIFE;
    // launcher acquisition preserves its entire response body.
    let prefix = shape::seeds::script::kernel_prefix_len(&fetched.kernel).unwrap();
    assert!(direct.kernel.as_slice() == &fetched.kernel[..prefix]);
    assert_eq!(
        direct.seed,
        shape::seeds::seed::seed_from_kernel(&direct.kernel).unwrap()
    );
    assert_eq!(
        direct.kernel_summary.sha256,
        seeds::parse_kernel(&direct.kernel).unwrap().sha256
    );
    let client = seeds::Arc::new(MockHttp {
        panic: true,
        calls: Default::default(),
    });
    let error = seeds::fetch(client, &url, seeds::FetchOptions::default()).unwrap_err();
    assert_eq!(error.code, "panic");
    assert!(error.message.contains("mock HTTP panic"));
}

#[test]
fn seeds_parse_registry_and_import_reject_invalid_inputs_without_writes() {
    let base = seeds::parse_url("https://example.invalid/path/launcher.js").unwrap();
    let url = seeds::resolve_url("../kernel.js?seed=public_token", Some(&base)).unwrap();
    assert_eq!(
        url.as_str(),
        "https://example.invalid/kernel.js?seed=public_token"
    );
    assert_eq!(
        seeds::token(url.as_str(), "seed").unwrap().as_str(),
        "public_token"
    );
    assert!(seeds::token("/kernel?seed=..%2Fprivate", "seed").is_err());
    assert!(seeds::parse_url("file:///private").is_err());
    assert!(seeds::parse(b"function {").is_err());
    assert!(seeds::parse_kernel(b"<html>not JavaScript</html>").is_err());
    for site in seeds::sites() {
        assert_eq!(seeds::site(site.name).unwrap().name, site.name);
    }
    assert!(seeds::site("unknown-public-fixture").is_err());
    let directory =
        std::env::temp_dir().join(format!("shape-sdk-invalid-import-{}", std::process::id()));
    let seed = seeds::token("/kernel?seed=public_token", "seed").unwrap();
    assert!(seeds::import(&directory, &seed, None, b"not a kernel").is_err());
    assert!(!directory.exists());
}
