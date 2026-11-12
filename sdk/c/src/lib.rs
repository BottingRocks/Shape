//! C ABI of the shape SDK. See `include/shape.h` for the contract.
//!
//! Command strings are NUL-terminated UTF-8; artifact loads use explicit byte
//! lengths and do not require a terminator. Results are JSON
//! envelopes, `{"ok":true,"value":…}` or `{"ok":false,"error":{"code","message"}}`,
//! allocated here and released with `shape_free`. Unwinding Rust panics during
//! calls and loads become `"panic"` error envelopes. Aborts (including allocation
//! failure) and invalid caller pointers cannot be recovered this way.

use std::ffi::{c_char, CStr, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::ptr;

use engine::sdk::{envelope, Error, Generator};

#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

/// Opaque loaded decode artifact.
pub struct ShapeGenerator(Generator);

static VERSION: &CStr =
    match CStr::from_bytes_with_nul(concat!(env!("CARGO_PKG_VERSION"), "\0").as_bytes()) {
        Ok(version) => version,
        Err(_) => panic!("version contains NUL"),
    };

fn owned(text: String) -> *mut c_char {
    // JSON never contains a raw NUL: serde_json escapes U+0000.
    CString::new(text)
        .unwrap_or_else(|_| {
            CString::new(
                r#"{"ok":false,"error":{"code":"failed","message":"result contained NUL"}}"#,
            )
            .unwrap()
        })
        .into_raw()
}

fn error_envelope(code: &'static str, message: impl Into<String>) -> String {
    envelope(Err(Error {
        code,
        message: message.into(),
    }))
}

/// Borrow a C string; `None` for NULL. Invalid UTF-8 is an error.
unsafe fn borrow<'a>(text: *const c_char) -> Result<Option<&'a str>, String> {
    if text.is_null() {
        return Ok(None);
    }
    CStr::from_ptr(text)
        .to_str()
        .map(Some)
        .map_err(|_| "argument is not UTF-8".to_string())
}

fn guarded(f: impl FnOnce() -> String) -> *mut c_char {
    owned(
        catch_unwind(AssertUnwindSafe(f))
            .unwrap_or_else(|_| error_envelope("panic", "panic in shape")),
    )
}

/// The SDK version. Static: do not free.
#[no_mangle]
pub extern "C" fn shape_version() -> *const c_char {
    VERSION.as_ptr()
}

/// Run one stateless command (`"deob"`, `"trace.program"`, …) with a JSON
/// request (NULL or "" means `{}`). Returns an envelope; free with `shape_free`.
///
/// # Safety
/// `command` and `request_json` are NULL or valid NUL-terminated strings.
#[no_mangle]
pub unsafe extern "C" fn shape_call(
    command: *const c_char,
    request_json: *const c_char,
) -> *mut c_char {
    guarded(|| match (borrow(command), borrow(request_json)) {
        (Ok(Some(command)), Ok(request)) => engine::sdk::call_json(command, request.unwrap_or("")),
        (Ok(None), _) => error_envelope("invalid_request", "command is NULL"),
        (Err(e), _) | (_, Err(e)) => error_envelope("invalid_request", e),
    })
}

/// Load and validate a decode artifact (`len` bytes of JSON). Returns NULL on
/// failure and, when `error_out` is not NULL, stores an error envelope there
/// (free it with `shape_free`).
///
/// # Safety
/// `artifact_json` points to `len` readable bytes in a single allocation;
/// `error_out` is NULL or writable and does not overlap those bytes.
#[no_mangle]
pub unsafe extern "C" fn shape_generator_load(
    artifact_json: *const u8,
    len: usize,
    error_out: *mut *mut c_char,
) -> *mut ShapeGenerator {
    if !error_out.is_null() {
        *error_out = ptr::null_mut();
    }
    let fail = |message: String, code: &'static str| {
        if !error_out.is_null() {
            *error_out = owned(error_envelope(code, message));
        }
        ptr::null_mut()
    };
    if artifact_json.is_null() {
        return fail("artifact is NULL".into(), "invalid_request");
    }
    // Rust slices cannot span more than isize::MAX bytes. Reject impossible
    // lengths before touching the pointer, even when it is non-NULL.
    if len > isize::MAX as usize {
        return fail(
            "artifact length exceeds isize::MAX".into(),
            "invalid_request",
        );
    }
    // Keep slice construction and the final handle allocation under the same
    // boundary guard as Generator::load (including its input copy).
    match catch_unwind(AssertUnwindSafe(|| {
        let bytes = std::slice::from_raw_parts(artifact_json, len);
        Generator::load(bytes).map(|generator| Box::into_raw(Box::new(ShapeGenerator(generator))))
    })) {
        Ok(Ok(generator)) => generator,
        Ok(Err(error)) => fail(error.message, error.code),
        Err(_) => fail("panic in shape".into(), "panic"),
    }
}

/// Call `"decode"`, `"explain"`, `"encode"` or `"schema"` on a loaded
/// generator. Safe to call from several threads at once.
///
/// # Safety
/// `generator` came from `shape_generator_load` and was not freed; strings are
/// NULL or valid NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn shape_generator_call(
    generator: *const ShapeGenerator,
    method: *const c_char,
    request_json: *const c_char,
) -> *mut c_char {
    guarded(|| {
        let Some(generator) = generator.as_ref() else {
            return error_envelope("invalid_request", "generator is NULL");
        };
        match (borrow(method), borrow(request_json)) {
            (Ok(Some(method)), Ok(request)) => generator.0.call_json(method, request.unwrap_or("")),
            (Ok(None), _) => error_envelope("invalid_request", "method is NULL"),
            (Err(e), _) | (_, Err(e)) => error_envelope("invalid_request", e),
        }
    })
}

/// Release a generator. NULL is ignored.
///
/// # Safety
/// `generator` came from `shape_generator_load` and is freed at most once.
/// No calls using this generator may be running when it is freed.
#[no_mangle]
pub unsafe extern "C" fn shape_generator_free(generator: *mut ShapeGenerator) {
    if !generator.is_null() {
        drop(Box::from_raw(generator));
    }
}

/// Release a string returned by this library. NULL is ignored.
///
/// # Safety
/// `text` is an owned result from this library and is freed at most once.
/// The static pointer returned by `shape_version` must not be passed here.
#[no_mangle]
pub unsafe extern "C" fn shape_free(text: *mut c_char) {
    if !text.is_null() {
        drop(CString::from_raw(text));
    }
}

#[cfg(test)]
mod tests;

/// Named operation dispatch, contract v2. Legacy shape_call stays unchanged.
/// # Safety
/// Strings follow shape_call's pointer and lifetime requirements.
#[no_mangle]
pub unsafe extern "C" fn shape_call_v2(
    command: *const c_char,
    request_json: *const c_char,
) -> *mut c_char {
    guarded(|| match (borrow(command), borrow(request_json)) {
        (Ok(Some(command)), Ok(request)) => {
            engine::sdk::call_v2_json(command, request.unwrap_or(""))
        }
        (Ok(None), _) => error_envelope("invalid_request", "command is NULL"),
        (Err(e), _) | (_, Err(e)) => error_envelope("invalid_request", e),
    })
}

/// Named alias of the legacy validated decode handle; same allocation and ABI.
pub type ShapeArtifact = ShapeGenerator;

// Keep compilation, conversion and allocation inside one panic boundary.
unsafe fn artifact_constructor(
    error_out: *mut *mut c_char,
    compile: impl FnOnce() -> engine::sdk::Result<Generator>,
) -> *mut ShapeArtifact {
    if !error_out.is_null() {
        *error_out = ptr::null_mut();
    }
    match catch_unwind(AssertUnwindSafe(|| {
        compile().map(|generator| Box::into_raw(Box::new(ShapeGenerator(generator))))
    })) {
        Ok(Ok(artifact)) => artifact,
        result => {
            let error = match result {
                Ok(Err(error)) => error_envelope(error.code, error.message),
                Err(_) => error_envelope("panic", "panic in shape"),
                Ok(Ok(_)) => unreachable!(),
            };
            if !error_out.is_null() {
                *error_out = owned(error);
            }
            ptr::null_mut()
        }
    }
}

/// Compile directly to a validated immutable handle, without document reload.
/// # Safety
/// request_json follows shape_call's string rules; error_out follows
/// shape_generator_load's writable slot and ownership rules.
#[no_mangle]
pub unsafe extern "C" fn shape_artifact_compile(
    request_json: *const c_char,
    error_out: *mut *mut c_char,
) -> *mut ShapeArtifact {
    artifact_constructor(error_out, || {
        let request = borrow(request_json).map_err(|message| Error {
            code: "invalid_request",
            message,
        })?;
        engine::sdk::compile_artifact_json(request.unwrap_or("")).map(Generator::from)
    })
}

/// # Safety
/// Same requirements as shape_generator_load.
#[no_mangle]
pub unsafe extern "C" fn shape_artifact_load(
    bytes: *const u8,
    len: usize,
    error_out: *mut *mut c_char,
) -> *mut ShapeArtifact {
    shape_generator_load(bytes, len, error_out)
}
/// # Safety
/// Same requirements as shape_generator_call.
#[no_mangle]
pub unsafe extern "C" fn shape_artifact_call(
    artifact: *const ShapeArtifact,
    method: *const c_char,
    request: *const c_char,
) -> *mut c_char {
    shape_generator_call(artifact, method, request)
}
/// # Safety
/// Same requirements as shape_generator_free.
#[no_mangle]
pub unsafe extern "C" fn shape_artifact_free(artifact: *mut ShapeArtifact) {
    shape_generator_free(artifact)
}

macro_rules! direct_operation {
    ($name:ident,$method:literal) => {
        /// Direct operation on an already validated artifact, with no reload.
        /// # Safety
        /// Same requirements as shape_artifact_call.
        #[no_mangle]
        pub unsafe extern "C" fn $name(
            artifact: *const ShapeArtifact,
            request: *const c_char,
        ) -> *mut c_char {
            shape_artifact_call(artifact, concat!($method, "\0").as_ptr().cast(), request)
        }
    };
}
direct_operation!(shape_decode, "decode");
direct_operation!(shape_encode, "encode");
direct_operation!(shape_explain, "explain");
direct_operation!(shape_schema, "schema");

/// Immutable loaded kernel, independent of any disk cache.
pub struct ShapeKernel(engine::sdk::LoadedKernel);
/// Parse and validate a kernel once.
/// # Safety
/// bytes/error_out follow shape_generator_load's pointer and lifetime rules.
#[no_mangle]
pub unsafe extern "C" fn shape_kernel_load(
    bytes: *const u8,
    len: usize,
    error_out: *mut *mut c_char,
) -> *mut ShapeKernel {
    if !error_out.is_null() {
        *error_out = ptr::null_mut()
    }
    let fail = |code: &'static str, message: String| {
        if !error_out.is_null() {
            *error_out = owned(error_envelope(code, message))
        }
        ptr::null_mut()
    };
    if bytes.is_null() || len > isize::MAX as usize {
        return fail("invalid_request", "invalid kernel bytes or length".into());
    }
    match catch_unwind(AssertUnwindSafe(|| {
        engine::sdk::LoadedKernel::load(std::slice::from_raw_parts(bytes, len))
    })) {
        Ok(Ok(kernel)) => Box::into_raw(Box::new(ShapeKernel(kernel))),
        Ok(Err(e)) => fail(e.code, e.message),
        Err(_) => fail("panic", "panic in shape".into()),
    }
}
/// # Safety
/// Kernel must remain live until all concurrent calls return; strings follow shape_call.
#[no_mangle]
pub unsafe extern "C" fn shape_kernel_call(
    kernel: *const ShapeKernel,
    operation: *const c_char,
    request: *const c_char,
) -> *mut c_char {
    guarded(|| {
        let Some(kernel) = kernel.as_ref() else {
            return error_envelope("invalid_request", "kernel is NULL");
        };
        match (borrow(operation), borrow(request)) {
            (Ok(Some(op)), Ok(r)) => kernel.0.call_json(op, r.unwrap_or("")),
            (Ok(None), _) => error_envelope("invalid_request", "operation is NULL"),
            (Err(e), _) | (_, Err(e)) => error_envelope("invalid_request", e),
        }
    })
}
/// # Safety
/// Free once, after all calls return. NULL is ignored.
#[no_mangle]
pub unsafe extern "C" fn shape_kernel_free(kernel: *mut ShapeKernel) {
    if !kernel.is_null() {
        drop(Box::from_raw(kernel))
    }
}
