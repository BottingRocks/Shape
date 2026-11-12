use super::*;
use serde_json::{json, Value};

const ARTIFACT: &[u8] = include_bytes!("../tests/fixtures/artifact.json");
const CAPTURE: &str = include_str!("../tests/fixtures/capture.json");

unsafe fn take(result: *mut c_char) -> Value {
    assert!(!result.is_null());
    let text = CStr::from_ptr(result).to_str().unwrap().to_owned();
    shape_free(result);
    serde_json::from_str(&text).unwrap()
}

fn success(envelope: Value) -> Value {
    assert_eq!(envelope["ok"], true, "{envelope}");
    assert!(envelope.get("error").is_none(), "{envelope}");
    envelope["value"].clone()
}

fn error(envelope: Value, code: &str) {
    assert_eq!(envelope["ok"], false, "{envelope}");
    assert_eq!(envelope["error"]["code"], code, "{envelope}");
    assert!(envelope["error"]["message"]
        .as_str()
        .is_some_and(|s| !s.is_empty()));
    assert!(envelope.get("value").is_none(), "{envelope}");
}

unsafe fn load() -> *mut ShapeGenerator {
    // No terminator is supplied, and overwriting then dropping the input must
    // not affect later calls through the returned handle.
    let mut bytes = ARTIFACT.to_vec();
    let previous = shape_call(ptr::null(), ptr::null());
    let mut error_out = previous;
    let generator = shape_generator_load(bytes.as_ptr(), bytes.len(), &mut error_out);
    assert!(!generator.is_null(), "{:?}", take(error_out));
    assert!(error_out.is_null());
    // load clears the slot but must not free its previous owned value.
    error(take(previous), "invalid_request");
    bytes.fill(0xff);
    generator
}

unsafe fn call(generator: *const ShapeGenerator, method: &CStr, request: Value) -> Value {
    let request = CString::new(request.to_string()).unwrap();
    take(shape_generator_call(
        generator,
        method.as_ptr(),
        request.as_ptr(),
    ))
}

#[test]
fn artifact_direct_type_validation_precedes_source_work() {
    for direct in [Value::Null, json!(1), json!("true"), json!([]), json!({})] {
        let request = CString::new(json!({"direct": direct}).to_string()).unwrap();
        let envelope = unsafe { take(shape_call(c"artifact.compile".as_ptr(), request.as_ptr())) };
        assert!(envelope["error"]["message"]
            .as_str()
            .unwrap()
            .contains("direct"));
        error(envelope, "invalid_request");
    }
}

#[test]
fn stateless_inputs_version_and_string_boundaries() {
    unsafe {
        let version = shape_version();
        assert!(!version.is_null());
        assert_eq!(version, shape_version());
        assert_eq!(
            CStr::from_ptr(version).to_str().unwrap(),
            env!("CARGO_PKG_VERSION")
        );
        for request in [ptr::null(), c"".as_ptr(), c" \n\t".as_ptr(), c"{}".as_ptr()] {
            assert_eq!(
                success(take(shape_call(c"version".as_ptr(), request)))["version"],
                json!(engine::sdk::VERSION)
            );
        }
        for request in [
            c"[".as_ptr(),
            c"[]".as_ptr(),
            c"null".as_ptr(),
            c"true".as_ptr(),
            c"1".as_ptr(),
            c"\"x\"".as_ptr(),
            c"\xff".as_ptr(),
        ] {
            error(
                take(shape_call(c"version".as_ptr(), request)),
                "invalid_request",
            );
        }
        for command in [ptr::null(), c"\xff".as_ptr()] {
            error(take(shape_call(command, ptr::null())), "invalid_request");
        }
        for command in [c"".as_ptr(), c"no-such-command".as_ptr()] {
            error(take(shape_call(command, ptr::null())), "unknown_command");
        }
        // C strings end at the first NUL. Escaped NULs remain JSON data and
        // output strings stay valid C strings (the serializer escapes them).
        let command = b"version\0ignored\0";
        let request = b"{}\0not JSON\0";
        success(take(shape_call(
            command.as_ptr().cast(),
            request.as_ptr().cast(),
        )));
        error(
            take(shape_call(
                c"report".as_ptr(),
                c"{\"source\":\"\",\"input_kind\":\"\\u0000\"}".as_ptr(),
            )),
            "invalid_request",
        );
        shape_free(ptr::null_mut());
        shape_generator_free(ptr::null_mut());
    }
}

#[test]
fn load_errors_initialize_slots_and_respect_explicit_lengths() {
    unsafe {
        for bytes in [
            b"".as_slice(),
            b"[",
            b"[]",
            b"null",
            b"{}",
            b"\xff",
            b"{}\0",
        ] {
            let previous = shape_call(ptr::null(), ptr::null());
            let mut error_out = previous;
            assert!(shape_generator_load(bytes.as_ptr(), bytes.len(), &mut error_out).is_null());
            error(take(error_out), "failed");
            error(take(previous), "invalid_request");
            assert!(shape_generator_load(bytes.as_ptr(), bytes.len(), ptr::null_mut()).is_null());
        }
        for (bytes, len) in [
            (ptr::null(), 0),
            (ptr::null(), 1),
            (b"x".as_ptr(), usize::MAX),
            (b"x".as_ptr(), isize::MAX as usize + 1),
        ] {
            let mut error_out = ptr::null_mut();
            assert!(shape_generator_load(bytes, len, &mut error_out).is_null());
            error(take(error_out), "invalid_request");
            assert!(shape_generator_load(bytes, len, ptr::null_mut()).is_null());
        }
        // A byte load must not silently truncate at NUL or ignore trailing bytes.
        let mut bytes = ARTIFACT.to_vec();
        bytes.extend_from_slice(b"\0garbage");
        let mut error_out = ptr::null_mut();
        assert!(shape_generator_load(bytes.as_ptr(), bytes.len(), &mut error_out).is_null());
        error(take(error_out), "failed");
        let generator = shape_generator_load(bytes.as_ptr(), ARTIFACT.len(), ptr::null_mut());
        assert!(!generator.is_null());
        shape_generator_free(generator);
    }
}

#[test]
fn generator_input_errors_and_independent_lifetimes() {
    unsafe {
        error(
            take(shape_generator_call(
                ptr::null(),
                c"schema".as_ptr(),
                c"{}".as_ptr(),
            )),
            "invalid_request",
        );
        for _ in 0..8 {
            let first = load();
            let second = load();
            assert_ne!(first, second);
            for method in [ptr::null(), c"\xff".as_ptr()] {
                error(
                    take(shape_generator_call(first, method, ptr::null())),
                    "invalid_request",
                );
            }
            for method in [c"".as_ptr(), c"unknown".as_ptr()] {
                error(
                    take(shape_generator_call(first, method, ptr::null())),
                    "unknown_command",
                );
            }
            for request in [
                c"[".as_ptr(),
                c"[]".as_ptr(),
                c"null".as_ptr(),
                c"false".as_ptr(),
                c"1".as_ptr(),
                c"\"x\"".as_ptr(),
                c"\xff".as_ptr(),
            ] {
                error(
                    take(shape_generator_call(first, c"schema".as_ptr(), request)),
                    "invalid_request",
                );
            }
            for method in [c"decode", c"encode", c"explain"] {
                error(call(first, method, json!({})), "invalid_request");
            }
            for request in [ptr::null(), c"".as_ptr(), c"{}".as_ptr()] {
                success(take(shape_generator_call(
                    first,
                    c"schema".as_ptr(),
                    request,
                )));
            }
            success(take(shape_generator_call(
                first,
                b"schema\0ignored\0".as_ptr().cast(),
                b"{}\0ignored\0".as_ptr().cast(),
            )));
            let result = shape_generator_call(first, c"schema".as_ptr(), ptr::null());
            shape_generator_free(first);
            success(take(result)); // results have no dependency on handle lifetime
            success(call(second, c"schema", json!({})));
            shape_generator_free(second);
        }
    }
}

#[test]
fn exported_generator_roundtrips_and_explains_real_signals() {
    unsafe {
        let generator = load();
        let capture: Value = serde_json::from_str(CAPTURE).unwrap();
        let decoded = success(call(
            generator,
            c"decode",
            json!({"capture": capture, "verify": true, "metadata": true}),
        ));
        assert_eq!(
            decoded["positionalSignals"],
            json!({
                "ConstantEmptyString": {"constant": ""},
                "DeviceMemory": {"deviceMemory": 32},
                "StorageAccessApiPresence": {"hasStorageAccess": true, "requestStorageAccess": false}
            })
        );
        assert_eq!(decoded["metadata"]["exactReencode"], true);
        let explained = success(call(
            generator,
            c"explain",
            json!({"decoded": decoded, "signal": "DeviceMemory", "verify": true}),
        ));
        assert_eq!(explained["status"], "decoded");
        assert_eq!(explained["fields"], json!({"deviceMemory": 32}));
        assert_eq!(
            explained["metadata"]["projectionCandidates"][0]["fields"][0]["numberBits"].as_u64(),
            Some(32_f64.to_bits()), // > 2^53; C JSON must preserve every digit
        );
        let encoded = success(call(generator, c"encode", json!({"decoded": decoded})));
        assert_eq!(encoded, capture);
        let encoded = success(call(
            generator,
            c"encode",
            json!({"signals": {"DeviceMemory": {"deviceMemory": 9}, "StorageAccessApiPresence": {"hasStorageAccess": false, "requestStorageAccess": true}}}),
        ));
        let changed = success(call(
            generator,
            c"decode",
            json!({"capture": encoded, "verify": true}),
        ));
        assert_eq!(
            changed["positionalSignals"]["DeviceMemory"]["deviceMemory"],
            9
        );
        assert_eq!(
            changed["positionalSignals"]["StorageAccessApiPresence"],
            json!({"hasStorageAccess": false, "requestStorageAccess": true})
        );
        error(
            call(
                generator,
                c"explain",
                json!({"decoded": decoded, "signal": "UnknownSignal"}),
            ),
            "invalid_request",
        );
        shape_generator_free(generator);
    }
}

#[test]
fn shared_handle_calls_finish_before_synchronized_free() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<ShapeGenerator>();
    unsafe {
        let generator = load();
        let shared = &*generator;
        let barrier = std::sync::Barrier::new(5);
        std::thread::scope(|scope| {
            for _ in 0..4 {
                let barrier = &barrier;
                scope.spawn(move || {
                    barrier.wait();
                    for _ in 0..4 {
                        success(call(shared, c"schema", json!({})));
                        let capture: Value = serde_json::from_str(CAPTURE).unwrap();
                        let decoded = success(call(shared, c"decode", json!({"capture": capture})));
                        assert_eq!(
                            decoded["positionalSignals"]["DeviceMemory"]["deviceMemory"],
                            32
                        );
                        success(take(shape_call(c"version".as_ptr(), ptr::null())));
                    }
                });
            }
            barrier.wait();
        }); // all calls joined; the handle is now quiescent
        shape_generator_free(generator);
    }
}

#[test]
fn boundary_guard_turns_unwinding_panics_into_owned_envelopes() {
    // No production command deliberately panics. Inject at the same guard used
    // by the exported call functions, without adding a public fault-injection ABI.
    unsafe {
        error(take(guarded(|| panic!("test boundary panic"))), "panic");
        success(take(shape_call(c"version".as_ptr(), ptr::null())));
    }
}

#[test]
fn named_artifact_aliases_reuse_validation_and_direct_operations() {
    unsafe {
        let mut error_out = ptr::null_mut();
        let artifact = shape_artifact_load(ARTIFACT.as_ptr(), ARTIFACT.len(), &mut error_out);
        assert!(!artifact.is_null());
        assert!(error_out.is_null());
        let exported = success(take(shape_artifact_call(
            artifact,
            c"artifact".as_ptr(),
            c"{}".as_ptr(),
        )));
        assert_eq!(exported, serde_json::from_slice::<Value>(ARTIFACT).unwrap());
        let request = CString::new(
            json!({"capture":serde_json::from_str::<Value>(CAPTURE).unwrap()}).to_string(),
        )
        .unwrap();
        let decoded = success(take(shape_decode(artifact, request.as_ptr())));
        let request = CString::new(json!({"decoded":decoded}).to_string()).unwrap();
        let encoded = success(take(shape_encode(artifact, request.as_ptr())));
        assert_eq!(encoded, serde_json::from_str::<Value>(CAPTURE).unwrap());
        let request =
            CString::new(json!({"decoded":decoded,"signal":"DeviceMemory"}).to_string()).unwrap();
        assert!(success(take(shape_explain(artifact, request.as_ptr()))).is_object());
        let schema = shape_schema(artifact, ptr::null());
        // Compatibility calls may share the very same allocation.
        assert_eq!(
            success(take(shape_generator_call(
                artifact,
                c"schema".as_ptr(),
                ptr::null()
            ))),
            success(take(shape_schema(artifact, ptr::null())))
        );
        shape_artifact_free(artifact);
        assert!(success(take(schema)).is_object());
        error(
            take(shape_decode(ptr::null(), ptr::null())),
            "invalid_request",
        );
        shape_artifact_free(ptr::null_mut());
    }
}

#[test]
fn named_dispatch_and_kernel_lifecycle_preserve_legacy_symbols() {
    const RAW: &str = include_str!("../../../tests/fixtures/javascript-vm-prefix.raw.js");
    unsafe {
        let version = success(take(shape_call_v2(c"version".as_ptr(), ptr::null())));
        assert_eq!(version["contract_version"], 2);
        error(
            take(shape_call_v2(ptr::null(), ptr::null())),
            "invalid_request",
        );
        error(
            take(shape_call_v2(c"version".as_ptr(), c"[]".as_ptr())),
            "invalid_request",
        );
        let request = CString::new(json!({"source":RAW}).to_string()).unwrap();
        let artifact = success(take(shape_call_v2(c"report".as_ptr(), request.as_ptr())));
        let mut bytes = artifact.to_string().into_bytes();
        let mut error_out = ptr::null_mut();
        let kernel = shape_kernel_load(bytes.as_ptr(), bytes.len(), &mut error_out);
        assert!(!kernel.is_null());
        assert!(error_out.is_null());
        bytes.fill(0xff);
        drop(bytes);
        let exported = success(take(shape_kernel_call(
            kernel,
            c"kernel.to_json".as_ptr(),
            ptr::null(),
        )));
        assert_eq!(
            serde_json::from_str::<Value>(exported.as_str().unwrap()).unwrap(),
            artifact
        );
        let result = shape_kernel_call(kernel, c"trace.program".as_ptr(), request.as_ptr());
        error(
            take(shape_kernel_call(
                kernel,
                c"trace.program".as_ptr(),
                c"{\"source\":\"different\"}".as_ptr(),
            )),
            "failed",
        );
        error(
            take(shape_kernel_call(kernel, c"unknown".as_ptr(), ptr::null())),
            "unknown_command",
        );
        shape_kernel_free(kernel);
        assert!(success(take(result))["complete"].is_boolean());
        assert!(shape_kernel_load(ptr::null(), 0, &mut error_out).is_null());
        error(take(error_out), "invalid_request");
        assert!(shape_kernel_load(ARTIFACT.as_ptr(), usize::MAX, &mut error_out).is_null());
        error(take(error_out), "invalid_request");
        error(
            take(shape_kernel_call(
                ptr::null(),
                c"trace.thread".as_ptr(),
                ptr::null(),
            )),
            "invalid_request",
        );
        shape_kernel_free(ptr::null_mut());
    }
}

#[test]
fn compile_handle_producer_reuses_validated_state_and_preserves_export() {
    const PROGRAM: &str = include_str!("../../../tests/fixtures/sdk/decode/program.js");
    let session: Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/sdk/decode/session.json"
    ))
    .unwrap();
    let capture: Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/sdk/decode/headers.json"
    ))
    .unwrap();
    let request =
        CString::new(json!({"source":PROGRAM,"session":session,"site_id":"example"}).to_string())
            .unwrap();
    unsafe {
        let previous = shape_call(ptr::null(), ptr::null());
        let mut error_out = previous;
        let artifact = shape_artifact_compile(request.as_ptr(), &mut error_out);
        assert!(!artifact.is_null(), "{:?}", take(error_out));
        assert!(error_out.is_null());
        error(take(previous), "invalid_request");
        let original = success(take(shape_artifact_call(
            artifact,
            c"artifact".as_ptr(),
            ptr::null(),
        )));
        // The compiled handle and explicit document operation use the same compiler.
        assert_eq!(
            success(take(shape_call_v2(
                c"artifact.compile".as_ptr(),
                request.as_ptr()
            ))),
            original
        );
        for _ in 0..3 {
            let decode = CString::new(json!({"capture":capture}).to_string()).unwrap();
            let decoded = success(take(shape_decode(artifact, decode.as_ptr())));
            let encode = CString::new(json!({"decoded":decoded}).to_string()).unwrap();
            assert_eq!(
                success(take(shape_encode(artifact, encode.as_ptr()))),
                capture
            );
        }
        let edit =
            CString::new(json!({"signals":{"DeviceMemory":{"deviceMemory":16}}}).to_string())
                .unwrap();
        let edited_capture = success(take(shape_encode(artifact, edit.as_ptr())));
        let decode = CString::new(json!({"capture":edited_capture}).to_string()).unwrap();
        let decoded = success(take(shape_decode(artifact, decode.as_ptr())));
        assert_eq!(
            decoded["positionalSignals"]["DeviceMemory"]["deviceMemory"],
            16
        );
        let explain =
            CString::new(json!({"decoded":decoded,"signal":"DeviceMemory"}).to_string()).unwrap();
        assert!(success(take(shape_explain(artifact, explain.as_ptr()))).is_object());
        assert_eq!(
            success(take(shape_artifact_call(
                artifact,
                c"artifact".as_ptr(),
                ptr::null()
            ))),
            original
        );
        let schema = shape_schema(artifact, ptr::null());
        shape_artifact_free(artifact);
        assert!(success(take(schema)).is_object());
        // A NULL error slot is also valid on success.
        let artifact = shape_artifact_compile(request.as_ptr(), ptr::null_mut());
        assert!(!artifact.is_null());
        shape_generator_free(artifact);
    }
}

#[test]
fn compile_handle_errors_and_panics_obey_load_ownership_rules() {
    unsafe {
        let bad_utf8 = [0xffu8, 0];
        let bad_json = CString::new("{").unwrap();
        let invalid = CString::new("[]").unwrap();
        let failed=CString::new(json!({"source":"function {","session":serde_json::from_str::<Value>(include_str!("../../../tests/fixtures/sdk/decode/session.json")).unwrap(),"site_id":"example"}).to_string()).unwrap();
        for (request, code) in [
            (ptr::null(), "invalid_request"),
            (bad_utf8.as_ptr().cast(), "invalid_request"),
            (bad_json.as_ptr(), "invalid_request"),
            (invalid.as_ptr(), "invalid_request"),
            (failed.as_ptr(), "failed"),
        ] {
            let previous = shape_call(ptr::null(), ptr::null());
            let mut error_out = previous;
            assert!(shape_artifact_compile(request, &mut error_out).is_null());
            error(take(error_out), code);
            error(take(previous), "invalid_request");
            assert!(shape_artifact_compile(request, ptr::null_mut()).is_null());
        }
        let mut error_out = ptr::null_mut();
        assert!(artifact_constructor(&mut error_out, || panic!("compile boundary test")).is_null());
        error(take(error_out), "panic");
        // Result calls use the same tested panic guard and remain usable afterwards.
        error(take(guarded(|| panic!("result boundary test"))), "panic");
        success(take(shape_call_v2(c"version".as_ptr(), ptr::null())));
    }
}
