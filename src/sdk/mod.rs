//! Named Rust operations and the compatibility surface SDK bindings call.
//!
//! Use the named operations and explicit [`LoadedArtifact`] inputs for Rust.
//! Cache policy is per call and defaults to [`CachePolicy::Disabled`].
//! The compatibility JSON interface [`call`] runs a stateless
//! command (`"deob"`, `"trace.program"`, `"signals.schema"`, …) and
//! [`Generator`] keeps one validated decode artifact loaded for repeated
//! `decode` / `explain` / `encode` / `schema`. The C ABI (`sdk/c`) wraps exactly these;
//! Python, Node and Go wrap the C ABI. Rust callers can use either this
//! module's named typed operations.
//!
//! Engine operations run on a [`crate::run_with_large_stack`] worker: host-language
//! threads have stacks far smaller than the tracer and parser need.
//!
//! Sources are text (`"source"`: raw capture or traced program JavaScript).
//! Requests must be JSON objects; an empty request string means `{}`. Nested
//! documents may be objects or JSON strings. Omitted or null optional fields
//! keep their defaults, except `artifact.compile.direct`, which requires a
//! boolean if present. Generator input errors use `invalid_request`, while
//! signal processing errors use `failed`.
//! Source and artifact inputs are documents, not paths. Legacy JSON `trace.program` may
//! read or write its kernel-artifact cache (`$SHAPE_KERNEL_CACHE`, `off`
//! disables it); `report_dir` selects that cache directory.

pub use crate::signals::decode::{
    CaptureHeaders, CompileOptions, DecodeArtifact, DecodeOptions, DecodedPayload,
};
use crate::signals::decode::{DecodeError, ValidatedArtifact};
pub use crate::signals::payload::extract::Session;
pub use crate::signals::wire::FloatByteOrder;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;

mod facade;
pub use facade::*;
mod extensions;
mod portable;
pub use extensions::{
    headers, pack, payload, seeds, unpack, unpack_canonical, Limits, Number, PackDecoded,
    PackValue, SerializerOptions,
};
pub use portable::{
    call_v2, call_v2_json, compile_artifact_json, LoadedKernel, OPERATION_CONTRACT,
};

/// The SDK version: the `shape` crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// A failed call: a stable machine-readable `code` and a message.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Error {
    /// `invalid_request`, `unknown_command`, `failed` or `panic`.
    pub code: &'static str,
    pub message: String,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

fn invalid(message: impl Into<String>) -> Error {
    Error {
        code: "invalid_request",
        message: message.into(),
    }
}
fn failed(message: impl std::fmt::Display) -> Error {
    Error {
        code: "failed",
        message: message.to_string(),
    }
}

/// The names [`call`] accepts, in documentation order.
pub const COMMANDS: &[&str] = &[
    "version",
    "deob",
    "report",
    "kernel.show",
    "ops.list",
    "ops.show",
    "trace.program",
    "trace.thread",
    "cleanup.thread",
    "naming.threads",
    "naming.collectors",
    "naming.plan",
    "naming.inventory",
    "signals.schema",
    "signals.show",
    "signals.inputs",
    "signals.contracts",
    "pin",
    "session.from_initializer",
    "artifact.compile",
    "artifact.compact_schema",
];

/// Run one stateless command on a large-stack worker.
///
/// A panic inside the library becomes an `Error { code: "panic" }` instead of
/// unwinding into the caller's language runtime.
pub fn call(command: &str, request: Value) -> Result<Value> {
    let command = command.to_owned();
    isolate(move || dispatch(&command, &request))
}

/// [`call`] with JSON text in and out: the shape of every binding.
pub fn call_json(command: &str, request: &str) -> String {
    envelope(parse_request(request).and_then(|request| call(command, request)))
}

/// `{"ok":true,"value":…}` or `{"ok":false,"error":{"code","message"}}`.
pub fn envelope(result: Result<Value>) -> String {
    match result {
        Ok(value) => json!({"ok": true, "value": value}).to_string(),
        Err(error) => json!({"ok": false, "error": error}).to_string(),
    }
}

fn parse_request(request: &str) -> Result<Value> {
    if request.trim().is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_str(request).map_err(|e| invalid(format!("request is not JSON: {e}")))
}

fn validate_request(request: &Value) -> Result<()> {
    if request.is_object() {
        Ok(())
    } else {
        Err(invalid("request must be a JSON object"))
    }
}

fn isolate<T: Send + 'static>(f: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
    crate::run_with_large_stack(move || {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).unwrap_or_else(|payload| {
            let message = payload
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "panic".into());
            Err(Error {
                code: "panic",
                message,
            })
        })
    })
}

// ---- request helpers ------------------------------------------------------

fn field<'a>(request: &'a Value, name: &str) -> Option<&'a Value> {
    request.get(name).filter(|v| !v.is_null())
}
fn text<'a>(request: &'a Value, name: &str) -> Result<&'a str> {
    field(request, name)
        .ok_or_else(|| invalid(format!("missing \"{name}\"")))?
        .as_str()
        .ok_or_else(|| invalid(format!("\"{name}\" must be a string")))
}
/// An optional string field: absent keeps the caller's default, but a value
/// of another JSON type is a bad request rather than something to ignore.
fn optional_text<'a>(request: &'a Value, name: &str) -> Result<Option<&'a str>> {
    match field(request, name) {
        None => Ok(None),
        Some(Value::String(text)) => Ok(Some(text)),
        Some(_) => Err(invalid(format!("\"{name}\" must be a string"))),
    }
}
fn flag(request: &Value, name: &str, default: bool) -> Result<bool> {
    match field(request, name) {
        None => Ok(default),
        Some(Value::Bool(b)) => Ok(*b),
        Some(_) => Err(invalid(format!("\"{name}\" must be true or false"))),
    }
}
fn number(request: &Value, name: &str) -> Result<Option<usize>> {
    match field(request, name) {
        None => Ok(None),
        Some(value) => {
            let number = value
                .as_u64()
                .ok_or_else(|| invalid(format!("\"{name}\" must be a non-negative integer")))?;
            usize::try_from(number)
                .map(Some)
                .map_err(|_| invalid(format!("\"{name}\" is too large")))
        }
    }
}
/// A nested JSON document, which may arrive as a document string (e.g. a
/// saved file). `source` stays text: it is never parsed as JSON.
fn document(request: &Value, name: &str) -> Result<Option<Value>> {
    match field(request, name) {
        None => Ok(None),
        Some(Value::String(text)) if name != "source" => serde_json::from_str::<Value>(text)
            .map(Some)
            .map_err(|e| invalid(format!("\"{name}\" is not JSON: {e}"))),
        Some(other) => Ok(Some(other.clone())),
    }
}
fn typed<T: DeserializeOwned>(request: &Value, name: &str) -> Result<Option<T>> {
    document(request, name)?
        .map(|value| serde_json::from_value(value).map_err(|e| invalid(format!("\"{name}\": {e}"))))
        .transpose()
}
fn to_value<T: Serialize>(value: &T) -> Result<Value> {
    serde_json::to_value(value).map_err(failed)
}
/// Library functions that render JSON text: hand back the parsed document.
fn parsed(json: String) -> Result<Value> {
    serde_json::from_str(&json).map_err(failed)
}
fn request_session(request: &Value) -> Result<Option<Session>> {
    typed(request, "session")
}
fn float_order(request: &Value) -> Result<Option<FloatByteOrder>> {
    match optional_text(request, "float_byte_order")? {
        None => Ok(None),
        Some("little") => Ok(Some(FloatByteOrder::Little)),
        Some("big") => Ok(Some(FloatByteOrder::Big)),
        Some(other) => Err(invalid(format!(
            "\"float_byte_order\" must be \"little\" or \"big\", got {other:?}"
        ))),
    }
}
/// `"overrides": {"fake": {"pc/state": true}, "real": ["pc/state"]}`.
fn overrides(request: &Value) -> Result<crate::BranchOverrides> {
    let mut overrides = crate::BranchOverrides::default();
    let Some(spec) = field(request, "overrides") else {
        return Ok(overrides);
    };
    let spec = spec
        .as_object()
        .ok_or_else(|| invalid("\"overrides\" must be an object"))?;
    if let Some(fake) = spec.get("fake").filter(|value| !value.is_null()) {
        let fake = fake
            .as_object()
            .ok_or_else(|| invalid("\"overrides.fake\" must be an object"))?;
        for (position, taken) in fake {
            let taken = taken
                .as_bool()
                .ok_or_else(|| invalid(format!("overrides.fake[{position}] must be a boolean")))?;
            overrides.fake.insert(
                crate::BranchOverrides::position(position).map_err(invalid)?,
                taken,
            );
        }
    }
    if let Some(real) = spec.get("real").filter(|value| !value.is_null()) {
        let real = real
            .as_array()
            .ok_or_else(|| invalid("\"overrides.real\" must be an array"))?;
        for position in real {
            let position = position
                .as_str()
                .ok_or_else(|| invalid("overrides.real entries must be \"pc/state\" strings"))?;
            overrides
                .real
                .insert(crate::BranchOverrides::position(position).map_err(invalid)?);
        }
    }
    Ok(overrides)
}
/// The seed name shared by `trace.program`, `signals.schema` and
/// `artifact.compile`.
fn name(request: &Value) -> Result<PathBuf> {
    Ok(PathBuf::from(
        optional_text(request, "name")?.unwrap_or("seed.js"),
    ))
}
/// `encode` takes a full decoded payload under `"decoded"` or bare named
/// signals under `"signals"`; either may arrive as a JSON document string.
fn encode_payload(request: &Value) -> Result<Value> {
    match (document(request, "decoded")?, document(request, "signals")?) {
        (Some(decoded), None) => Ok(decoded),
        (None, Some(signals)) => Ok(json!({"positionalSignals": signals})),
        _ => Err(invalid(
            "encode needs exactly one of \"decoded\" or \"signals\"",
        )),
    }
}

// ---- commands -------------------------------------------------------------

fn dispatch(command: &str, request: &Value) -> Result<Value> {
    validate_request(request)?;
    match command {
        "version" => Ok(json!({"version": VERSION, "commands": COMMANDS})),
        "deob" => dispatch_deob(request),
        "report" => {
            let source = text(request, "source")?;
            let limits = crate::PrefixLimits::default();
            let artifact = match optional_text(request, "input_kind")? {
                None | Some("raw") => crate::KernelArtifact::compile_source(source, limits),
                Some("deobfuscated") => {
                    crate::KernelArtifact::compile_from_deobfuscated(source, limits)
                }
                Some(other) => return Err(invalid(format!("unknown input_kind {other:?}"))),
            }
            .map_err(failed)?;
            parsed(artifact.to_json().map_err(failed)?)
        }
        "kernel.show" => {
            let standardized = standardized(request)?;
            let properties = crate::extract_actual_properties(&standardized).map_err(failed)?;
            let operations: Vec<String> = typed(request, "operations")?.unwrap_or_default();
            properties
                .validate_selected_operations(&operations)
                .map_err(invalid)?;
            parsed(properties.tracer_json(&operations))
        }
        "ops.list" | "ops.show" => dispatch_ops(command, request),
        "trace.program" => trace_program(request),
        "trace.thread" => trace_thread(request),
        "cleanup.thread" => {
            let cleaned =
                crate::emitted_cleanup::cleanup_thread(text(request, "source")?).map_err(failed)?;
            Ok(json!({
                "code": cleaned.code,
                "cleanup": {"inlined_bindings": cleaned.inlined_bindings, "inlined_snapshots": cleaned.inlined_snapshots,
                    "rounds": cleaned.rounds, "inlined_helpers": cleaned.inlined_helper_calls,
                    "removed_helpers": cleaned.removed_helpers, "remaining_helpers": cleaned.remaining_helpers,
                    "property_writes": cleaned.rewritten_property_writes, "arrays": cleaned.folded_arrays,
                    "methods": cleaned.simplified_method_calls, "loops": cleaned.reconstructed_loops,
                    "flattened_blocks": cleaned.flattened_blocks}
            }))
        }
        "naming.threads" => {
            let reference: Option<crate::naming::labels::LabelReference> =
                typed(request, "labels")?;
            let named = crate::naming::name_threads_with_labels(
                text(request, "source")?,
                flag(request, "rename", false)?,
                reference.as_ref(),
            )
            .map_err(failed)?;
            Ok(json!({"source": named.source, "report": to_value(&named.report)?}))
        }
        "naming.collectors" => {
            let found = crate::naming::collectors::identify_collectors(
                text(request, "source")?,
                flag(request, "rename", false)?,
            )
            .map_err(failed)?;
            Ok(json!({"source": found.source, "report": to_value(&found.report)?}))
        }
        "naming.plan" => to_value(
            &crate::naming::semantic_names::plan_semantic_names(text(request, "source")?)
                .map_err(failed)?,
        ),
        "naming.inventory" => {
            let source = text(request, "source")?;
            let index =
                crate::naming::function_index::extract_function_index(source).map_err(failed)?;
            let graph =
                crate::naming::role_graph::extract_program_role_graph(source).map_err(failed)?;
            Ok(json!({"function_index": to_value(&index)?, "role_graph": to_value(&graph)?}))
        }
        "signals.schema" => {
            let options = crate::signals::analysis::SignalFinderOptions {
                layouts_only: flag(request, "layouts_only", false)?,
                finder_roles: flag(request, "finder_roles", true)?,
            };
            // Validate the seed stem before spending time on analysis.
            let seed = optional_text(request, "name")?.map(|n| {
                std::path::Path::new(n)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(n)
                    .to_string()
            });
            let mut analysis = crate::signals::analysis::analyze_program_signals(
                text(request, "source")?,
                &options,
            )
            .map_err(failed)?;
            analysis.report.seed = seed;
            to_value(&analysis.report)
        }
        "signals.show" => to_value(
            &crate::signals::extract::scanner::extract_program_properties(text(request, "source")?)
                .map_err(failed)?,
        ),
        "signals.inputs" => to_value(
            &crate::naming::inputs_context::extract_program_inputs_with_context(text(
                request, "source",
            )?)
            .map_err(failed)?,
        ),
        "signals.contracts" => to_value(
            &crate::signals::extract::contracts::extract_signal_contracts(text(request, "source")?)
                .map_err(failed)?,
        ),
        "pin" => {
            let source = text(request, "source")?;
            let init = optional_text(request, "init")?;
            let session = match (request_session(request)?, init) {
                (Some(session), None) => session,
                (None, Some(init)) => {
                    crate::signals::payload::extract::session_from_initializer(init)
                        .map_err(failed)?
                }
                _ => return Err(invalid("pin needs exactly one of \"session\" or \"init\"")),
            };
            let pinned = crate::signals::payload::extract::pin(source, &session).map_err(failed)?;
            to_value(&pinned.layout)
        }
        "session.from_initializer" => to_value(
            &crate::signals::payload::extract::session_from_initializer(text(request, "init")?)
                .map_err(failed)?,
        ),
        "artifact.compile" => artifact_compile(request),
        "artifact.compact_schema" => {
            let artifact: DecodeArtifact =
                typed(request, "artifact")?.ok_or_else(|| invalid("missing \"artifact\""))?;
            artifact.validate().map_err(failed)?;
            to_value(&artifact.compact_schema().map_err(failed)?)
        }
        other => Err(Error {
            code: "unknown_command",
            message: format!("unknown command {other:?}; known: {}", COMMANDS.join(", ")),
        }),
    }
}

/// `deob`: `{"source"}` → `{"code"}`; with `"artifact": true` also the
/// compiled source-free kernel artifact (as `shape deob --report`).
fn dispatch_deob(request: &Value) -> Result<Value> {
    let source = text(request, "source")?;
    if !flag(request, "artifact", false)? {
        return Ok(json!({"code": crate::deobfuscate(source).map_err(failed)?}));
    }
    let report = crate::deobfuscate_with_report(source).map_err(failed)?;
    let artifact =
        crate::KernelArtifact::compile_from_raw(source, &report, crate::PrefixLimits::default())
            .map_err(failed)?;
    Ok(json!({
        "code": report.output,
        "artifact": parsed(artifact.to_json().map_err(failed)?)?,
    }))
}

fn standardized(request: &Value) -> Result<String> {
    let source = text(request, "source")?;
    match optional_text(request, "input_kind")? {
        None | Some("raw") => crate::deobfuscate(source).map_err(failed),
        Some("deobfuscated") => Ok(source.to_owned()),
        Some(other) => Err(invalid(format!("unknown input_kind {other:?}"))),
    }
}

fn dispatch_ops(command: &str, request: &Value) -> Result<Value> {
    use crate::operations::view::{
        available_keys, handler_by_key, ops_list_json, ops_show_json, Capture, OpsView,
    };
    // `ops.show` validates its key before the (expensive) source analysis, and
    // rejects keys that do not fit the u32 operation space instead of truncating.
    let key = match command {
        "ops.list" => None,
        _ => {
            let op = number(request, "op")?.ok_or_else(|| invalid("missing \"op\""))?;
            Some(
                u32::try_from(op)
                    .map_err(|_| invalid(format!("\"op\" must be at most {}", u32::MAX)))?,
            )
        }
    };
    let source = text(request, "source")?;
    let digest = crate::source::sha256_hex(source.as_bytes());
    let report = crate::deobfuscate_with_report(source).map_err(failed)?;
    let catalogue = crate::catalogue_from_report(&report).map_err(failed)?;
    let view = OpsView {
        catalogue: &catalogue,
        capture: Capture {
            input_kind: "raw",
            digest: &digest,
            byte_length: source.len() as u64,
            raw_identity: "verified",
        },
        span_source: None,
        analyses: None,
        annotation: None,
    };
    if command == "ops.list" {
        return parsed(ops_list_json(&view));
    }
    let key = key.ok_or_else(|| invalid("missing \"op\""))?;
    let handler = handler_by_key(&catalogue, key).ok_or_else(|| {
        invalid(format!(
            "unknown operation key {key}; available: {:?}",
            available_keys(&catalogue)
        ))
    })?;
    parsed(ops_show_json(&view, handler))
}

/// `trace.program`: `{"source", "name"?, "steps"?, "thread_cleanup"?,
/// "no_cleanup"?, "check_reuse"?, "overrides"?, "jobs"?, "report_dir"?}` →
/// `{"code", "traced", "stopped": [{"function","reason"}], "kernel_globals", "complete"}`.
fn trace_program(request: &Value) -> Result<Value> {
    let options = crate::trace::program::LinkOptions {
        seed: name(request)?,
        report_dir: optional_text(request, "report_dir")?.map(PathBuf::from),
        steps: number(request, "steps")?.unwrap_or(1_000_000),
        dump_stages: None,
        thread_cleanup: flag(request, "thread_cleanup", false)?,
        no_cleanup: flag(request, "no_cleanup", false)?,
        check_reuse: flag(request, "check_reuse", false)?,
        overrides: overrides(request)?,
        jobs: number(request, "jobs")?,
        quiet: flag(request, "quiet", true)?,
    };
    let linked =
        crate::trace::program::link_source(text(request, "source")?, &options).map_err(failed)?;
    Ok(json!({
        "code": linked.code,
        "complete": linked.stopped.is_empty(),
        "traced": linked.traced,
        "stopped": linked.stopped.iter().map(|(function, reason)| json!({"function": function, "reason": reason})).collect::<Vec<_>>(),
        "kernel_globals": linked.kernel_globals,
    }))
}

/// `trace.thread`: `{"source" | "artifact", "pc", "state", "descriptor"?,
/// "initial_slots"?, "inputs"?, "steps"?, "self_check"?, "outline"?,
/// "reverse_sides"?, "partial"?, "overrides"?}`.
fn trace_thread(request: &Value) -> Result<Value> {
    let artifact = match (
        field(request, "artifact"),
        field(request, "source").is_some(),
    ) {
        (Some(artifact), false) => Some(artifact),
        (None, true) => None,
        _ => {
            return Err(invalid(
                "trace.thread needs exactly one of \"source\" or \"artifact\"",
            ))
        }
    };
    // Validate every option before compiling a source artifact: a missing pc
    // must not pay for deobfuscation and tracing.
    let options = crate::trace::thread::ThreadOptions {
        pc: number(request, "pc")?.ok_or_else(|| invalid("missing \"pc\""))?,
        state: number(request, "state")?.ok_or_else(|| invalid("missing \"state\""))?,
        descriptor: number(request, "descriptor")?,
        initial_slots: typed(request, "initial_slots")?,
        inputs: typed(request, "inputs")?,
        steps: number(request, "steps")?.unwrap_or(1_000_000),
        self_check: flag(request, "self_check", false)?,
        outline: flag(request, "outline", true)?,
        reverse_sides: flag(request, "reverse_sides", false)?,
        partial: flag(request, "partial", false)?,
        overrides: overrides(request)?,
    };
    let artifact = match artifact {
        Some(artifact) => {
            let json = match artifact {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            crate::KernelArtifact::from_json(&json).map_err(failed)?
        }
        None => crate::trace::thread::compile_kernel_artifact(text(request, "source")?)
            .map_err(failed)?,
    };
    let walked = crate::trace::thread::walk_thread(&artifact, &options).map_err(failed)?;
    Ok(json!({
        "code": walked.code,
        "complete": walked.completed,
        "stop": walked.report.stop.to_string(),
        "steps": walked.report.records.len(),
        "handlers_traced": walked.report.trace.handlers_traced,
        "undeclared": walked.undeclared,
        "self_check": walked.report.self_check,
        "source_hash": artifact.source_hash(),
    }))
}

/// `artifact.compile`: `{"source" (traced program, or raw seed with
/// "input_kind": "raw"), "session", "site_id", "name"?, "float_byte_order"?, "direct"?}`
/// → the reusable decode artifact (as `shape decode --save-artifact`).
/// `float_byte_order` defaults to `"little"`; omitting it must still decode
/// floating-point signals.
/// `direct` is a strictly typed boolean, default false (null is invalid). It
/// opts into source-format decoding without lifecycle certification; artifacts
/// retain that provenance when loaded by a generator.
fn artifact_compile(request: &Value) -> Result<Value> {
    let options = artifact_compile_options(request)?;
    let source = text(request, "source")?;
    let session = request_session(request)?.ok_or_else(|| invalid("missing \"session\""))?;
    let site_id = text(request, "site_id")?.to_owned();
    // Validate the cheap request shape before any source work.
    let order = float_order(request)?.unwrap_or(FloatByteOrder::Little);
    let seed = name(request)?;
    let program = match optional_text(request, "input_kind")? {
        None | Some("program") => source.to_owned(),
        Some("raw") => {
            let linked = crate::trace::program::link_source(
                source,
                &crate::trace::program::LinkOptions {
                    seed,
                    report_dir: None,
                    steps: 1_000_000,
                    dump_stages: None,
                    thread_cleanup: true,
                    no_cleanup: false,
                    check_reuse: false,
                    overrides: Default::default(),
                    jobs: None,
                    quiet: true,
                },
            )
            .map_err(failed)?;
            if !linked.stopped.is_empty() {
                return Err(failed(
                    "seed tracing stopped; inspect with trace.program before compiling",
                ));
            }
            linked.code
        }
        Some(other) => return Err(invalid(format!("unknown input_kind {other:?}"))),
    };
    let artifact =
        DecodeArtifact::compile_with_options(&program, &session, site_id, order, options)
            .map_err(failed)?;
    to_value(&artifact)
}

fn artifact_compile_options(request: &Value) -> Result<CompileOptions> {
    let direct = match request.get("direct") {
        None => false,
        Some(Value::Bool(direct)) => *direct,
        Some(_) => return Err(invalid("\"direct\" must be true or false")),
    };
    Ok(CompileOptions { direct })
}

// ---- generator ------------------------------------------------------------

/// Caller input errors have the same code for every generator method.
/// Signal decoding and other processing errors stay `failed`.
fn decode_error(error: DecodeError) -> Error {
    match error {
        DecodeError::Input(message) => invalid(message),
        other => failed(other),
    }
}

/// One loaded decode artifact: validated once, then decoded many times.
///
/// Cheap to clone and safe to share across threads.
#[derive(Clone)]
pub struct Generator {
    artifact: Arc<ValidatedArtifact>,
}

impl Generator {
    /// Parse and validate a saved decode artifact (`artifact.compile` output
    /// or `shape decode --save-artifact`).
    pub fn load(artifact_json: &[u8]) -> Result<Self> {
        let bytes = artifact_json.to_vec();
        isolate(move || {
            Ok(Self {
                artifact: Arc::new(ValidatedArtifact::from_json(&bytes).map_err(failed)?),
            })
        })
    }

    /// The decode artifact this generator holds.
    pub fn artifact(&self) -> &DecodeArtifact {
        self.artifact.artifact()
    }

    /// Descriptive seed schema, with the same result as the JSON `schema`
    /// method. This export contains no session secrets or executable recipes
    /// and cannot be loaded as a decode artifact.
    pub fn schema(&self) -> Result<crate::signals::decode::CompactSeedSchema> {
        let artifact = self.artifact.clone();
        isolate(move || artifact.artifact().compact_schema().map_err(failed))
    }

    /// Headers → named signals. Per-signal details and verification are opt-in;
    /// `metadata.originalReport` always retains the bytes for explain and encode.
    pub fn decode(
        &self,
        capture: &CaptureHeaders,
        session: Option<&Session>,
        options: DecodeOptions,
    ) -> Result<DecodedPayload> {
        let (artifact, capture, session) =
            (self.artifact.clone(), capture.clone(), session.cloned());
        isolate(move || {
            artifact
                .decode(&capture, session.as_ref(), options)
                .map_err(decode_error)
        })
    }

    /// One signal's full metadata, built on demand from a decoded payload.
    pub fn explain(
        &self,
        decoded: &DecodedPayload,
        signal: &str,
        session: Option<&Session>,
        verify: bool,
    ) -> Result<crate::signals::wire::NamedSignal> {
        let (artifact, decoded, signal, session) = (
            self.artifact.clone(),
            decoded.clone(),
            signal.to_owned(),
            session.cloned(),
        );
        isolate(move || {
            if decoded.site_id != artifact.artifact().site_id {
                return Err(invalid("payload/artifact siteId mismatch"));
            }
            artifact
                .explain(&decoded, &signal, session.as_ref(), verify)
                .map_err(decode_error)
        })
    }

    /// Named signals → headers. Accepts a full decoded payload (its retained
    /// original report supplies channels, seeds and per-signal prefixes) or
    /// just `positionalSignals`.
    pub fn encode(&self, payload: &Value, session: Option<&Session>) -> Result<CaptureHeaders> {
        let (artifact, payload, session) =
            (self.artifact.clone(), payload.clone(), session.cloned());
        isolate(move || {
            artifact
                .encode_named(&payload, session.as_ref())
                .map_err(decode_error)
        })
    }

    /// Encode a typed decoded payload, retaining its original report just as
    /// [`Self::encode`] does for the equivalent JSON document. Edit
    /// `positional_signals` before calling to change named fields.
    pub fn encode_decoded(
        &self,
        decoded: &DecodedPayload,
        session: Option<&Session>,
    ) -> Result<CaptureHeaders> {
        let (artifact, decoded, session) =
            (self.artifact.clone(), decoded.clone(), session.cloned());
        isolate(move || {
            artifact
                .encode_named(&to_value(&decoded)?, session.as_ref())
                .map_err(decode_error)
        })
    }

    /// `decode` / `explain` / `encode` / `schema` with JSON requests, as the bindings call them:
    /// - `decode`: `{"capture", "session"?, "verify"?, "metadata"?}`
    /// - `explain`: `{"decoded", "signal", "session"?, "verify"?}`
    /// - `encode`: `{"signals" | "decoded", "session"?}`; either document may
    ///   be a JSON string, as with `typed()`.
    /// - `schema`: `{}`.
    pub fn call(&self, method: &str, request: Value) -> Result<Value> {
        validate_request(&request)?;
        match method {
            "artifact" => to_value(self.artifact()),
            "decode" => {
                let capture: CaptureHeaders =
                    typed(&request, "capture")?.ok_or_else(|| invalid("missing \"capture\""))?;
                let options = DecodeOptions {
                    verify: flag(&request, "verify", false)?,
                    metadata: flag(&request, "metadata", false)?,
                };
                to_value(&self.decode(&capture, request_session(&request)?.as_ref(), options)?)
            }
            "explain" => {
                let decoded: DecodedPayload =
                    typed(&request, "decoded")?.ok_or_else(|| invalid("missing \"decoded\""))?;
                to_value(&self.explain(
                    &decoded,
                    text(&request, "signal")?,
                    request_session(&request)?.as_ref(),
                    flag(&request, "verify", false)?,
                )?)
            }
            "encode" => {
                let payload = encode_payload(&request)?;
                to_value(&self.encode(&payload, request_session(&request)?.as_ref())?)
            }
            "schema" => to_value(&self.schema()?),
            other => Err(Error {
                code: "unknown_command",
                message: format!(
                    "unknown generator method {other:?}; known: decode, explain, encode, schema"
                ),
            }),
        }
    }

    /// [`Generator::call`] with JSON text in and out.
    pub fn call_json(&self, method: &str, request: &str) -> String {
        envelope(parse_request(request).and_then(|request| self.call(method, request)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generator_is_shareable_across_threads() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Generator>();
    }

    #[test]
    fn panic_fence_contains_payloads_and_leaves_later_calls_usable() {
        for panic_with_string in [false, true] {
            let error = isolate::<()>(move || {
                assert_eq!(std::thread::current().name(), Some("shape"));
                if panic_with_string {
                    std::panic::panic_any(String::from("SDK panic probe"));
                }
                panic!("SDK panic probe");
            })
            .unwrap_err();
            assert_eq!(error.code, "panic");
            assert_eq!(error.message, "SDK panic probe");
        }
        let error = isolate::<()>(|| std::panic::panic_any(17u8)).unwrap_err();
        assert_eq!(error.code, "panic");
        assert_eq!(error.message, "panic");
        assert!(call("version", json!({})).is_ok());
    }

    #[test]
    fn requests_and_errors_use_one_envelope() {
        let version: Value = serde_json::from_str(&call_json("version", "")).unwrap();
        assert_eq!(version["ok"], true);
        assert_eq!(version["value"]["version"], VERSION);
        let unknown: Value = serde_json::from_str(&call_json("nope", "{}")).unwrap();
        assert_eq!(unknown["ok"], false);
        assert_eq!(unknown["error"]["code"], "unknown_command");
        let bad: Value = serde_json::from_str(&call_json("deob", "{")).unwrap();
        assert_eq!(bad["error"]["code"], "invalid_request");
        let missing: Value = serde_json::from_str(&call_json("deob", "{}")).unwrap();
        assert_eq!(missing["error"]["code"], "invalid_request");
    }

    #[test]
    fn cleanup_thread_runs_without_files() {
        let out = call(
            "cleanup.thread",
            json!({"source": "function f(a) { let $SSA_1 = a; return $SSA_1; }"}),
        )
        .unwrap();
        assert!(out["code"].as_str().unwrap().contains("return"));
    }

    fn invalid_call(command: &str, request: Value) -> Error {
        call(command, request).expect_err("request should be rejected")
    }

    #[test]
    fn artifact_direct_requires_boolean_before_source_work() {
        for direct in [
            Value::Null,
            json!(0),
            json!(1),
            json!("true"),
            json!([]),
            json!({}),
        ] {
            // Even missing source/session cannot mask a malformed direct option.
            let error = invalid_call("artifact.compile", json!({"direct": direct}));
            assert_eq!(error.code, "invalid_request");
            assert!(error.message.contains("\"direct\""), "{error}");
        }
        assert!(!artifact_compile_options(&json!({})).unwrap().direct);
        assert!(
            !artifact_compile_options(&json!({"direct": false}))
                .unwrap()
                .direct
        );
        assert!(
            artifact_compile_options(&json!({"direct": true}))
                .unwrap()
                .direct
        );
    }

    #[test]
    fn artifact_compile_forwards_direct_and_preserves_default() {
        let source = include_str!("../../tests/fixtures/sdk/decode/program.js");
        let session: Session =
            serde_json::from_str(include_str!("../../tests/fixtures/sdk/decode/session.json"))
                .unwrap();
        let request = json!({"source": source, "session": session, "site_id": "example"});
        let strict = call("artifact.compile", request.clone()).unwrap();
        let mut explicit = request.clone();
        explicit["direct"] = json!(false);
        assert_eq!(call("artifact.compile", explicit).unwrap(), strict);
        let expected = crate::run_with_large_stack(move || {
            DecodeArtifact::compile_with_options(
                source,
                &session,
                "example".into(),
                FloatByteOrder::Little,
                CompileOptions { direct: true },
            )
            .unwrap()
        });
        let mut direct_request = request;
        direct_request["direct"] = json!(true);
        let direct = call("artifact.compile", direct_request).unwrap();
        assert_eq!(direct, serde_json::to_value(expected).unwrap());
        assert_ne!(
            direct, strict,
            "direct provenance must survive serialization"
        );
        let generator = Generator::load(&serde_json::to_vec(&direct).unwrap()).unwrap();
        let capture =
            serde_json::from_str(include_str!("../../tests/fixtures/sdk/decode/headers.json"))
                .unwrap();
        let decoded = generator
            .decode(
                &capture,
                None,
                DecodeOptions {
                    verify: true,
                    metadata: true,
                },
            )
            .unwrap();
        assert!(decoded.metadata.exact_reencode);
    }

    #[test]
    fn wrong_typed_optional_fields_are_invalid_requests() {
        let session = json!({
            "transactionToken": "ab",
            "alphabet": "AB",
            "shuffleKey": [],
            "keys": [0, 0, 0, 0, 0, 0, 0, 0],
        });
        let cases = [
            (
                "report",
                json!({"source": "", "input_kind": 1}),
                "input_kind",
            ),
            (
                "kernel.show",
                json!({"source": "", "input_kind": false}),
                "input_kind",
            ),
            (
                "artifact.compile",
                json!({"source": "", "session": session, "site_id": "example",
                    "input_kind": 1}),
                "input_kind",
            ),
            (
                "artifact.compile",
                json!({"source": "", "session": session, "site_id": "example",
                    "float_byte_order": 1}),
                "float_byte_order",
            ),
            ("trace.program", json!({"source": "", "name": 1}), "name"),
            (
                "trace.program",
                json!({"source": "", "report_dir": []}),
                "report_dir",
            ),
            ("signals.schema", json!({"source": "", "name": []}), "name"),
            ("pin", json!({"source": "", "init": {}}), "init"),
        ];
        for (command, request, field) in cases {
            let error = invalid_call(command, request);
            assert_eq!(
                error.code, "invalid_request",
                "{command}: {}",
                error.message
            );
            assert!(
                error.message.contains(field),
                "{command}: {} does not name {field}",
                error.message
            );
        }
    }

    #[test]
    fn ops_show_rejects_keys_above_u32() {
        let error = invalid_call(
            "ops.show",
            json!({"source": "", "op": u64::from(u32::MAX) + 1}),
        );
        assert_eq!(error.code, "invalid_request");
        assert!(error.message.contains("op"), "{}", error.message);
    }

    #[test]
    fn trace_thread_validates_options_before_compiling() {
        // Compiling this source would fail; the missing option must win.
        let error = invalid_call("trace.thread", json!({"source": ""}));
        assert_eq!(error.code, "invalid_request");
        assert!(error.message.contains("\"pc\""), "{}", error.message);

        let error = invalid_call("trace.thread", json!({"source": "", "pc": 0}));
        assert_eq!(error.code, "invalid_request");
        assert!(error.message.contains("\"state\""), "{}", error.message);

        let error = invalid_call(
            "trace.thread",
            json!({"source": "", "pc": 0, "state": 0, "initial_slots": "["}),
        );
        assert_eq!(error.code, "invalid_request");
        assert!(error.message.contains("initial_slots"), "{}", error.message);

        // The exactly-one-of check is unchanged.
        let error = invalid_call("trace.thread", json!({"pc": 0, "state": 0}));
        assert_eq!(error.code, "invalid_request");
        assert!(error.message.contains("exactly one"), "{}", error.message);
    }

    #[test]
    fn overrides_require_object_shapes() {
        for overrides in [json!(true), json!({"fake": []}), json!({"real": {}})] {
            let error = invalid_call(
                "trace.program",
                json!({"source": "", "overrides": overrides}),
            );
            assert_eq!(error.code, "invalid_request");
            assert!(error.message.contains("overrides"), "{}", error.message);
        }
        // Per-entry validation is unchanged.
        for overrides in [json!({"fake": {"1/2": 1}}), json!({"real": [1]})] {
            let error = invalid_call(
                "trace.program",
                json!({"source": "", "overrides": overrides}),
            );
            assert_eq!(error.code, "invalid_request");
            assert!(error.message.contains("overrides"), "{}", error.message);
        }
        // trace.thread validates overrides before compiling, too.
        let error = invalid_call(
            "trace.thread",
            json!({"source": "", "pc": 0, "state": 0, "overrides": {"fake": 3}}),
        );
        assert_eq!(error.code, "invalid_request");
        // Valid entries still parse as pc/state positions.
        let parsed =
            overrides(&json!({"overrides": {"fake": {"1/2": true}, "real": ["3/4"]}})).unwrap();
        assert_eq!(parsed.fake.get(&(1, 2)), Some(&true));
        assert!(parsed.real.contains(&(3, 4)));
    }

    #[test]
    fn encode_accepts_document_strings() {
        let signals = encode_payload(&json!({
            "signals": "{\"DeviceMemory\":{\"deviceMemory\":9}}"
        }))
        .unwrap();
        assert_eq!(
            signals,
            json!({"positionalSignals": {"DeviceMemory": {"deviceMemory": 9}}})
        );
        let decoded = encode_payload(&json!({"decoded": "{\"positionalSignals\":{}}"})).unwrap();
        assert_eq!(decoded, json!({"positionalSignals": {}}));
        for request in [json!({"signals": "not json"}), json!({"decoded": "{"})] {
            let error = encode_payload(&request).unwrap_err();
            assert_eq!(error.code, "invalid_request");
        }
        let error = encode_payload(&json!({"decoded": {}, "signals": {}})).unwrap_err();
        assert_eq!(error.code, "invalid_request");
    }

    #[test]
    fn generator_input_errors_are_invalid_requests() {
        let missing = decode_error(DecodeError::Input(
            "signal X is missing from this payload".into(),
        ));
        assert_eq!(missing.code, "invalid_request");
        let failed = decode_error(DecodeError::Signal {
            position: 0,
            reason: "unexpected bytes".into(),
        });
        assert_eq!(failed.code, "failed");
    }

    #[test]
    fn requests_must_be_objects_even_for_commands_without_fields() {
        for request in [json!(null), json!([]), json!(true), json!(1), json!("{}")] {
            let error = call("version", request.clone()).unwrap_err();
            assert_eq!(error.code, "invalid_request");
            assert!(error.message.contains("JSON object"));
            let envelope: Value =
                serde_json::from_str(&call_json("version", &request.to_string())).unwrap();
            assert_eq!(envelope["error"]["code"], "invalid_request");
        }
        assert!(call("version", json!({})).is_ok());
        assert_eq!(
            serde_json::from_str::<Value>(&call_json("version", " \n ")).unwrap()["ok"],
            true
        );
    }
}
