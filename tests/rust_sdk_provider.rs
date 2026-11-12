//! Portable provider v2 contract, checked against the named Rust operations.
use serde_json::{json, Value};
use shape::sdk;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
const RAW: &str = include_str!("fixtures/javascript-vm-prefix.raw.js");
fn call(id: &str, r: Value) -> Value {
    sdk::call_v2(id, r).unwrap()
}
fn temp() -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    std::env::temp_dir().join(format!(
        "shape-provider-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}
#[test]
fn contract_enumeration_and_request_boundary() {
    let contract: Value = serde_json::from_str(sdk::OPERATION_CONTRACT).unwrap();
    let version = call("version", json!({}));
    assert_eq!(version["contract_version"], 2);
    let mut ids = std::collections::BTreeSet::new();
    for op in contract["operations"].as_array().unwrap() {
        let id = op["id"].as_str().unwrap();
        assert!(ids.insert(id));
        assert!(version["commands"].as_array().unwrap().contains(&json!(id)));
        let outcome = sdk::call_v2(id, json!({}));
        if let Err(e) = outcome {
            assert_ne!(e.code, "unknown_command", "{id}: {}", e.message);
        }
    }
    assert!(ids.len() > 60);
    for r in [json!([]), json!(null), json!(42)] {
        assert_eq!(
            sdk::call_v2("version", r).unwrap_err().code,
            "invalid_request"
        );
    }
    assert_eq!(
        sdk::call_v2("invented", json!({})).unwrap_err().code,
        "unknown_command"
    );
    for operation in ["headers.invented", "payload.invented", "seeds.invented"] {
        assert_eq!(
            sdk::call_v2(operation, json!({})).unwrap_err().code,
            "unknown_command"
        );
    }
    assert_eq!(
        sdk::call("version", json!({})).unwrap()["commands"]
            .as_array()
            .unwrap()
            .len(),
        21
    );
}
#[test]
fn cleanup_defaults_cache_validation_and_source_identity() {
    let expected =
        serde_json::to_value(sdk::trace::program(RAW, Default::default()).unwrap()).unwrap();
    assert_eq!(call("trace.program", json!({"source":RAW})), expected);
    assert_eq!(
        call(
            "trace.program",
            json!({"source":RAW,"thread_cleanup":true,"no_cleanup":false})
        ),
        expected
    );
    let diagnostic = serde_json::to_value(
        sdk::trace::program(
            RAW,
            sdk::ProgramOptions {
                no_cleanup: true,
                ..Default::default()
            },
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        call("trace.program", json!({"source":RAW,"no_cleanup":true})),
        diagnostic
    );
    for cache in [
        json!(false),
        json!({}),
        json!({"mode":"unknown"}),
        json!({"mode":"directory"}),
        json!({"mode":"directory","directory":""}),
        json!({"mode":"disabled","directory":"oops"}),
    ] {
        let error = sdk::call_v2(
            "trace.program",
            json!({"source":"invalid source","cache":cache}),
        )
        .unwrap_err();
        assert_eq!(error.code, "invalid_request");
        assert!(
            error.message.contains("cache")
                || error.message.contains("mode")
                || error.message.contains("directory")
        );
    }
    for field in ["steps", "jobs"] {
        let mut r = json!({"source":RAW});
        r[field] = json!(0);
        assert_eq!(
            sdk::call_v2("trace.program", r).unwrap_err().code,
            "invalid_request"
        );
    }
    let before = std::env::var_os("SHAPE_KERNEL_CACHE");
    let cache = temp();
    let artifact = call(
        "trace.corpus.load",
        json!({"source":RAW,"name":"source.js","cache":{"mode":"directory","directory":cache}}),
    );
    assert!(cache.join("source.js.kernel.json").is_file());
    let bytes = artifact.to_string();
    let handle = sdk::LoadedKernel::load(bytes.as_bytes()).unwrap();
    std::fs::remove_dir_all(&cache).unwrap();
    assert_eq!(
        handle.call("trace.program", json!({"source":RAW})).unwrap(),
        expected
    );
    assert!(!cache.exists());
    assert!(handle
        .call("trace.program", json!({"source":format!("{RAW}\n")}))
        .is_err());
    let changed = format!("{RAW}\n");
    // Put the old identity back at the same cache key: it must be rejected.
    std::fs::create_dir_all(&cache).unwrap();
    std::fs::write(cache.join("source.js.kernel.json"), bytes).unwrap();
    let stale = sdk::call_v2(
        "trace.corpus.load",
        json!({"source":changed,"name":"source.js","cache":{"mode":"directory","directory":cache}}),
    )
    .unwrap_err();
    assert_eq!(stale.code, "failed");
    assert!(stale.message.contains("another seed"));
    std::fs::remove_file(cache.join("source.js.kernel.json")).unwrap();
    let replacement = call(
        "trace.corpus.load",
        json!({"source":changed,"name":"source.js","cache":{"mode":"directory","directory":cache}}),
    );
    assert_ne!(replacement, artifact);
    std::fs::remove_dir_all(&cache).unwrap();
    assert_eq!(std::env::var_os("SHAPE_KERNEL_CACHE"), before);
    assert_eq!(call("kernel.load", json!({"artifact":artifact})), artifact);
}
#[test]
fn pack_and_byte_framing_preserve_numeric_and_byte_domains() {
    let value = json!({"kind":"array","value":[{"kind":"number","value":"8000000000000000"},{"kind":"binary","value":[0,128,255]}]});
    let bytes = call("pack", json!({"value":value}));
    let decoded = call("unpack", json!({"bytes":bytes}));
    assert_eq!(decoded["value"], value);
    assert_eq!(decoded["original_bytes"], bytes);
    assert_eq!(call("unpack_canonical", json!({"bytes":bytes})), value);
    assert_eq!(
        sdk::call_v2("pack", json!({"value":{"kind":"number","value":123}}))
            .unwrap_err()
            .code,
        "invalid_request"
    );
    assert_eq!(
        sdk::call_v2("unpack", json!({"bytes":[256]}))
            .unwrap_err()
            .code,
        "invalid_request"
    );
    let profile: Value =
        serde_json::from_str(include_str!("fixtures/sdk/generator/profile.json")).unwrap();
    let header_profile = call("headers.codec", json!({"profile":profile}));
    assert_eq!(header_profile["format"], profile["format"]);
    let mut bare = profile.clone();
    for f in [
        "header_name",
        "transaction_token",
        "chunk_size",
        "header_context",
        "positional_count",
        "keysets",
    ] {
        bare.as_object_mut().unwrap().remove(f);
    }
    let bytes = json!([0, 1, 128, 255]);
    let encoded = call(
        "payload.encode_bytes",
        json!({"profile":bare,"bytes":bytes,"seeds":[4294967295u32,23]}),
    );
    let decoded = call(
        "payload.decode_bytes",
        json!({"profile":bare,"text":encoded}),
    );
    let domain: sdk::payload::Payload = serde_json::from_value(decoded).unwrap();
    assert_eq!(domain.plaintext().unwrap(), vec![0, 1, 128, 255]);
    let payload = sdk::payload::Payload::raw([u32::MAX, 23], vec![0, 1, 128, 255]);
    let encoded = call(
        "headers.encode_bytes",
        json!({"profile":profile,"payload":payload}),
    );
    let decoded = call(
        "headers.decode_bytes",
        json!({"profile":profile,"headers":encoded}),
    );
    assert_eq!(decoded, serde_json::to_value(payload).unwrap());
}
#[test]
fn corpus_and_nonserde_domain_projections() {
    let sources = json!([{"name":"one","source":"(function main(){ var registry=[1,2,3]; function entry(){return registry[0];} return {init:entry}; })();"}]);
    let analysis = call("naming.corpus.analyze", json!({"sources":sources}));
    assert_eq!(analysis[0]["name"], "one");
    assert!(analysis[0]["plan"].is_object());
    assert!(analysis[0]["rename"]["source"].is_string());
    let difference = call(
        "naming.corpus.compare",
        json!({"baseline":sources,"current":sources}),
    );
    assert_eq!(difference[0]["name_divergences"], json!([]));
    assert_eq!(
        call("trace.corpus.compare", json!({"baseline":{},"current":{}})),
        json!({"regressions":[],"changes":[]})
    );
    assert_eq!(
        call(
            "trace.corpus.compare_inline",
            json!({"outlined":{},"inline":{}})
        ),
        json!([])
    );
    let classification = call("ops.classify.source", json!({"source":RAW}));
    assert!(classification["variant"].is_string());
    assert!(classification["ops"].is_array());
    assert_eq!(
        call("ops.classify.report", json!({"source":RAW})),
        classification
    );
    let first = classification["ops"][0]["handler_index"].clone();
    let template = call(
        "ops.classify.template",
        json!({"source":RAW,"handler_index":first}),
    );
    assert_eq!(template["handler_index"], first);
    let report = shape::deobfuscate_with_report(RAW).unwrap();
    assert_eq!(
        call("ops.classify.report", json!({"report":report})),
        classification
    );
    let templates = shape::operations::templates::handler_templates_from_report(&report).unwrap();
    let handler = &templates.handlers[0];
    let source =
        &report.output[handler.source_span.start as usize..handler.source_span.end as usize];
    assert_eq!(
        call(
            "ops.classify.template",
            json!({"template":handler,"source":source})
        ),
        template
    );
    let kinds = call("ops.classify.seeds", json!({"seeds":[classification]}));
    assert!(kinds.is_array());
    let sites = call("seeds.sites", json!({}));
    assert!(!sites.as_array().unwrap().is_empty());
    let first = &sites[0];
    assert_eq!(call("seeds.site", json!({"name":first["name"]})), *first);
    assert!(call("seeds.client", json!({}))["timeout_ms"].is_u64());
    assert_eq!(
        call(
            "seeds.resolve_url",
            json!({"reference":"/kernel.js?seed=abc","base":"https://example.test/launcher.js"})
        ),
        "https://example.test/kernel.js?seed=abc"
    );
    assert!(!call("signals.catalog", json!({}))
        .as_array()
        .unwrap()
        .is_empty());
}

#[test]
fn compile_to_loaded_state_matches_document_operation_and_reuses_state() {
    let request = json!({
        "source":include_str!("fixtures/sdk/decode/program.js"),
        "session":serde_json::from_str::<Value>(include_str!("fixtures/sdk/decode/session.json")).unwrap(),
        "site_id":"example"
    });
    let loaded = sdk::compile_artifact_json(&request.to_string()).unwrap();
    let clone = loaded.clone();
    let generator = sdk::Generator::from(loaded);
    // Both owners borrow the exact same validated artifact allocation.
    assert!(std::ptr::eq(clone.artifact(), generator.artifact()));
    let exported = generator.call("artifact", json!({})).unwrap();
    assert_eq!(exported, call("artifact.compile", request));
    let capture: Value =
        serde_json::from_str(include_str!("fixtures/sdk/decode/headers.json")).unwrap();
    for _ in 0..3 {
        let decoded = generator
            .call("decode", json!({"capture":capture}))
            .unwrap();
        assert_eq!(
            generator
                .call("encode", json!({"decoded":decoded}))
                .unwrap(),
            capture
        );
    }
    assert_eq!(generator.call("artifact", json!({})).unwrap(), exported);
    for request in [
        "",
        "{",
        "[]",
        "null",
        "{}",
        r#"{"source":"","direct":null}"#,
    ] {
        let error = sdk::compile_artifact_json(request).err().unwrap();
        assert_eq!(error.code, "invalid_request", "{request}");
    }
}
