# Rust

Use `shape::sdk` to decode captured headers, inspect collected signals, edit
named fields, and encode headers again. The same operation names lead to
tracing, kernel inspection, signal analysis and naming. Each operation takes
its own inputs; there is no global loaded seed or client initialization step.

Choose the task that matches your input:

- With captured headers and a decode artifact, [decode, edit and encode](#decode-explain-and-encode).
- With a raw seed, [trace and compile](#workflow) in memory; saving outputs is optional.
- For investigation, call [signal](#inspect-signals), [kernel or tracing](#recover-and-investigate-code) operations directly. Each takes its own inputs.

[Shared SDK documents](../README.md#documents) describe capture/session formats.
The [signal editing walkthrough](../signals.md) shows decoded field values.

**Contents:** [Decode, explain and encode](#decode-explain-and-encode) ·
[Workflow](#workflow) · [Artifacts and reuse](#artifacts-and-reuse) ·
[Explicit cache policy](#explicit-cache-policy) · [Inspect signals](#inspect-signals) ·
[Recover and investigate code](#recover-and-investigate-code) ·
[Advanced capabilities](#advanced-capabilities) · [Install and build](#install-and-build) ·
[API](#api) · [Errors](#errors) · [Threading and lifetime](#threading-and-lifetime) ·
[Performance](#performance)

## Decode, explain and encode

Inputs are `decode-artifact.json` from `sdk::artifact::compile` and captured
header documents from the same build/site. Each capture document has the form
`{"siteId":"example.test","headers":{"a":"…"}}`: use your actual site ID
and captured values. Header names are suffixes, including numeric chunks such
as `a0`; the artifact supplies their common prefix. Ellipses are explanatory,
not valid encoded header values.

This complete program accepts capture filenames as command-line arguments,
loads and validates the artifact **once before the loop**, and writes one
re-encoded header document beside each input. It explains the first available
named signal without building detailed metadata for every signal.

```rust
use shape::sdk::{self, CaptureHeaders, DecodeOptions};
use std::{env, error::Error, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let captures: Vec<PathBuf> = env::args_os().skip(1).map(PathBuf::from).collect();
    if captures.is_empty() {
        return Err("pass one or more capture JSON filenames".into());
    }
    let artifact = sdk::artifact::load(&fs::read("decode-artifact.json")?)?;
    for input in captures {
        let capture: CaptureHeaders = serde_json::from_slice(&fs::read(&input)?)?;
        let decoded = sdk::decode(&artifact, &capture, None, DecodeOptions::default())?;
        for (name, fields) in &decoded.positional_signals {
            println!("{name}: {}", serde_json::to_string_pretty(fields)?);
        }
        if let Some(name) = decoded.positional_signals.keys().next() {
            let detail = sdk::explain(&artifact, &decoded, name, None, false)?;
            println!("{name}: {:?}", detail.status);
        }
        let encoded = sdk::encode(&artifact, &decoded, None)?;
        fs::write(
            input.with_extension("reencoded.json"),
            serde_json::to_vec_pretty(&encoded)?,
        )?;
    }
    Ok(())
}
```

Keep the loaded artifact outside capture loops: each decode reuses its
validated recipes. Your application reads and parses capture JSON. Saving
the artifact is optional.

### Choose decode detail

`DecodeOptions::default()` returns named fields and always retains
`decoded.metadata.original_report` (`metadata.originalReport` in JSON).
Keep that full payload for later edits and re-encoding.

| Need | Option or operation |
| --- | --- |
| Details for one signal | `explain` computes them on demand |
| Details for every signal | `metadata: true` computes them eagerly |
| Byte-identical signal/header verification | `verify: true` checks re-encoding |

Both options default to `false`. `metadata.exact_reencode` records whether
verification ran; a default decode does not claim verified equality.

### Edit a field and preserve the capture

To edit a known field, this independent program requires a capture containing
`DeviceMemory.deviceMemory`. It preserves the full decoded payload, including
the original report, and saves edited headers. Choose a supported field
present in your own capture when that signal is absent.

```rust
use shape::sdk::{self, CaptureHeaders, DecodeOptions};
use std::{error::Error, fs};

fn main() -> Result<(), Box<dyn Error>> {
    let artifact = sdk::artifact::load(&fs::read("decode-artifact.json")?)?;
    let capture: CaptureHeaders = serde_json::from_slice(&fs::read("headers.json")?)?;
    let mut decoded = sdk::decode(
        &artifact,
        &capture,
        None,
        DecodeOptions {
            verify: true,
            metadata: false,
        },
    )?;
    let fields = decoded
        .positional_signals
        .get_mut("DeviceMemory")
        .ok_or("capture has no supported DeviceMemory signal")?;
    let memory = fields
        .get_mut("deviceMemory")
        .ok_or("deviceMemory field missing")?;
    *memory = serde_json::json!(8);
    let headers = sdk::encode(&artifact, &decoded, None)?;
    fs::write("headers.edited.json", serde_json::to_vec_pretty(&headers)?)?;
    Ok(())
}
```

An unchanged full payload preserves the captured header values when artifact,
session and supported encoding match. Full-payload encode retains channels,
seeds, keysets, header context and each signal's four-byte random prefix.
Edited values must survive encode/decode exactly or encoding fails. Encode
checks its output even when the original decode used `verify: false`.

`sdk::encode_signals(&artifact, &signals, None)` accepts `sdk::NamedSignals`
when constructing fields from scratch. It uses fresh prefixes and artifact
layout defaults; it does not preserve the original capture's envelope.

The artifact's session is the default. A `Some(&session)` override replaces
the entire session; use the same override for decode, explain and encode.
Site IDs must agree. Arbitrary headers from another build cannot be made
compatible by changing their `siteId` label.

## Workflow

When you have no decode artifact, supply three matching files:

| Input | Contents |
| --- | --- |
| `seed.js` | Raw captured VM JavaScript |
| `init.js` | Matching initializer JavaScript containing the session initialization |
| `headers.json` | A real capture document as described above |

The following program traces the raw seed with **cache disabled**, enables
thread cleanup for finder-compatible emitted code, checks completion, derives
the session, and compiles executable decode recipes. It then inspects and
conditionally edits `DeviceMemory`, and encodes the full payload. Naming and
renaming are separate investigation tools; no naming pass is required here.

```rust
use shape::sdk::{
    self, ArtifactOptions, CachePolicy, CaptureHeaders, DecodeOptions, ProgramInput, ProgramOptions,
};
use std::{error::Error, fs};

fn main() -> Result<(), Box<dyn Error>> {
    let raw_seed = fs::read_to_string("seed.js")?;
    let initializer = fs::read_to_string("init.js")?;
    let capture: CaptureHeaders = serde_json::from_slice(&fs::read("headers.json")?)?;
    let traced = sdk::trace::program(
        &raw_seed,
        ProgramOptions {
            name: "seed.js".into(),
            cache: CachePolicy::Disabled,
            thread_cleanup: true,
            no_cleanup: false,
            jobs: None,
            ..Default::default()
        },
    )?;
    if !traced.complete {
        return Err(format!("incomplete trace: {:?}", traced.stopped).into());
    }
    fs::write("program.js", &traced.code)?;
    let session = sdk::session::from_initializer(&initializer)?;
    let artifact = sdk::artifact::compile(
        ProgramInput::Program(&traced.code),
        &session,
        &capture.site_id,
        ArtifactOptions {
            cache: CachePolicy::Disabled,
            ..Default::default()
        },
    )?;
    // These writes are deliberate persistence, not required SDK state.
    fs::write("decode-artifact.json", artifact.to_json()?)?;
    fs::write(
        "seed-schema.json",
        serde_json::to_vec_pretty(&sdk::schema(&artifact)?)?,
    )?;
    let mut decoded = sdk::decode(&artifact, &capture, None, DecodeOptions::default())?;
    println!("{} named signals", decoded.positional_signals.len());
    if let Some(fields) = decoded.positional_signals.get_mut("DeviceMemory") {
        if let Some(memory) = fields.get_mut("deviceMemory") {
            *memory = serde_json::json!(8);
        }
    }
    let headers = sdk::encode(&artifact, &decoded, None)?;
    fs::write("headers.edited.json", serde_json::to_vec_pretty(&headers)?)?;
    Ok(())
}
```

### Check completion and choose tracing controls

Check `traced.complete` before using the program. A stopped trace can contain
code without being complete.

| Control | When to use it |
| --- | --- |
| `thread_cleanup: true` | Clean emitted threads for signal extraction |
| `no_cleanup: false` | Keep full finishing, including parameter/capture normalization and kernel-value substitution |
| `jobs: None` / `Some(n)` | Use available cores / cap concurrency |
| `no_cleanup: true` | Inspect diagnostic output with only lighter per-thread cleanup |

`artifact::compile` extracts the signals needed for recipes. Separate signal
reports, deobfuscation exports and semantic naming are optional investigation.

### Compile from source or reuse a program

- `ProgramInput::Program` consumes traced mini-JS without retracing.
- `ProgramInput::Raw(&raw_seed)` traces and compiles in one call using
  `ArtifactOptions`. Use the expanded example for tracing controls and
  completion metadata.

Omit the file writes for an entirely in-memory pipeline. Disabled caching
still permits explicit output writes.

### Compile with relaxed source support

Compilation is strict by default (`ArtifactOptions.direct = false`). Setting
`direct: true` explicitly permits source-format compilation without lifecycle
and strict algorithm certification. Unsupported constructs can still fail;
direct mode does not certify strict consumer support. Its provenance is
persisted in the decode artifact and honored when loaded. Exact verification
still checks the resulting signal/header round trip.

## Artifacts and reuse

| Item | Produced by | Consumed by; reuse boundary |
| --- | --- | --- |
| Raw seed, `seed.js` | Caller capture or explicit seed acquisition | `deob`, `report(KernelSource::Raw)`, raw tracing and raw compile |
| Deobfuscated source, `seed.deob.js` | `deob(...).code` | `report(KernelSource::Deobfuscated)` and `kernel::show`; readable source, not a compiled runtime |
| Compiled kernel, `seed.kernel.json` | Optional `deob(...).artifact` or `report`; `kernel::to_json` serializes | `kernel::load`, kernel getters, `trace::thread(KernelInput::Artifact)`, `trace::program_with_artifact` with matching raw source; skips kernel recompilation |
| Traced mini-JS, `program.js` | Complete `trace::program(...).code` | `ProgramInput::Program`, signal analysis, naming; avoids VM tracing, later analysis still parses source |
| Pinned seed layout | `pin(program, SessionInput)` | Structural report-slot/session layout for inspection and lower-level framing; not executable signal recipes |
| Detailed source schema | `signals::schema(program, SchemaOptions)` | Rich signal/layout/diagnostic analysis for inspection; not a loaded decoder |
| Compact descriptive schema, `seed-schema.json` | `schema(&loaded)`, `artifact::compact_schema(document)`, or `signals::compact_schema(program, CompileOptions)` | Documentation and schema consumers; omits executable recipes and session data |
| Executable decode artifact, `decode-artifact.json` | `artifact::compile(...).to_json()` | `artifact::load` once → `LoadedArtifact` for repeated decode/explain/encode; includes layout, session defaults and wire recipes |

A kernel artifact cannot replace a decode artifact. A schema cannot decode.
Pinned layout and kernel layout describe different things: report slots versus
VM runtime facts. Only supported canonical artifact formats are accepted;
there is no arbitrary source/AST image format or implicit loaded state.

A loaded kernel avoids JSON loading and kernel compilation, but each thread
walk still prepares runtime state. Whole-program reuse additionally requires
matching raw source and checks its identity. Kernel getters expose bytecode,
descriptors, templates and entry facts; they cannot reconstruct every
source-only `kernel::show` or `ops::*` projection.

## Explicit cache policy

New named Rust tracing/compilation operations default to
`CachePolicy::Disabled`. To opt into a disk cache, choose a directory per
operation. This independent recipe reads `seed.js`, caches only its compiled
kernel, and saves a complete program:

```rust
use shape::sdk::{self, CachePolicy, ProgramOptions};
use std::{error::Error, fs};

fn main() -> Result<(), Box<dyn Error>> {
    let raw_seed = fs::read_to_string("seed.js")?;
    let traced = sdk::trace::program(
        &raw_seed,
        ProgramOptions {
            name: "seed.js".into(),
            cache: CachePolicy::Directory("kernel-cache".into()),
            thread_cleanup: true,
            no_cleanup: false,
            jobs: None,
            ..Default::default()
        },
    )?;
    if !traced.complete {
        return Err(format!("incomplete trace: {:?}", traced.stopped).into());
    }
    fs::write("program.js", traced.code)?;
    Ok(())
}
```

On a miss, kernel compilation runs and the explicit directory receives
`kernel-cache/seed.js.kernel.json` (the `name` filename plus `.kernel.json`).
On a matching hit, the kernel loads instead of being recompiled. The VM trace
still runs. Invalid/unreadable explicit entries, mismatched source identity
and publication failures are errors; a corresponding legacy `.report.json`
is not a supported kernel cache entry. Directory caches can be prepopulated
with canonical kernel JSON for the same source/name.

Set `cache: CachePolicy::Disabled` to bypass all kernel cache reads and writes,
including environment/default locations. Policies are per call, so independent
callers can choose different behavior. Neither tracing results, naming results
nor parsed ASTs are cached by this policy. Reusing `LoadedArtifact` in memory
and explicitly saving a file are separate choices.

Advanced opt-in: `CachePolicy::Environment` selects legacy cache resolution
(`SHAPE_KERNEL_CACHE`, then platform/user cache defaults). The legacy CLI and
JSON dispatch retain their previous behavior. No process environment changes
are necessary to select or disable caching in the named Rust API.

## Inspect signals

### Find a signal's meaning

The catalog is source-independent documentation. This program counts available
cards and saves the `DeviceMemory` card, including its logical field descriptions
and examples. Those examples are documentation, not observations of your capture
or measured encoded sizes.

```rust
use shape::sdk;
use std::{error::Error, fs};

fn main() -> Result<(), Box<dyn Error>> {
    let catalog = sdk::signals::catalog()?;
    println!("{} documented signals", catalog.len());
    let card = sdk::signals::describe("DeviceMemory")?;
    println!("{}", serde_json::to_string_pretty(&card["summary"])?);
    println!("{}", serde_json::to_string_pretty(&card["logical_fields"])?);
    fs::write("device-memory.card.json", serde_json::to_vec_pretty(&card)?)?;
    Ok(())
}
```

Names are canonical and case-sensitive. For capture-backed values use
`decode`/`explain`; a documented signal can be absent, unsupported or have no
data in a particular build. These calls do not promise every capture-backed
size/provenance field available in the CLI's signal inspection view.

### Inspect the current seed's structure

Read a complete `program.js` from the workflow. Save separate reports so each
has a clear purpose: properties describe the program, inputs identify external
context, contracts describe signal extraction, and schema reports layout and
diagnostics. These source operations parse/analyze their input; saving the
program avoids retracing, not all subsequent analysis.

```rust
use shape::sdk::{self, CompileOptions, SchemaOptions};
use std::{error::Error, fs};

fn main() -> Result<(), Box<dyn Error>> {
    let program = fs::read_to_string("program.js")?;
    let properties = sdk::signals::show(&program)?;
    fs::write(
        "program.properties.json",
        serde_json::to_vec_pretty(&properties)?,
    )?;
    let inputs = sdk::signals::inputs(&program)?;
    fs::write("program.inputs.json", serde_json::to_vec_pretty(&inputs)?)?;
    let contracts = sdk::signals::contracts(&program)?;
    fs::write(
        "program.contracts.json",
        serde_json::to_vec_pretty(&contracts)?,
    )?;
    let schema = sdk::signals::schema(
        &program,
        SchemaOptions {
            name: Some("seed.js".into()),
            ..Default::default()
        },
    )?;
    fs::write("program.schema.json", serde_json::to_vec_pretty(&schema)?)?;
    let compact = sdk::signals::compact_schema(&program, CompileOptions::default())?;
    fs::write("seed-schema.json", serde_json::to_vec_pretty(&compact)?)?;
    Ok(())
}
```

Choose options for the report you need:

- `SchemaOptions`: detailed analysis, with `layouts_only`, `finder_roles` and an optional seed name.
- `CompileOptions { direct }`: compact descriptive schema; no session or capture needed.
- `ArtifactOptions`: executable decode recipes, as in the [workflow](#workflow).

Contract reports support typed signal lookup. Paired-trace provenance analysis
and CLI rendering controls are separate lower-level operations.

### Pin the layout for a session

Use a matching initializer to resolve report slots and cipher/session settings.
This independent recipe exports that pinned layout for inspection. Decode
artifact compilation already performs its required pinning; running this
recipe first is optional.

```rust
use shape::sdk::{self, SessionInput};
use std::{error::Error, fs};

fn main() -> Result<(), Box<dyn Error>> {
    let program = fs::read_to_string("program.js")?;
    let initializer = fs::read_to_string("init.js")?;
    let session = sdk::session::from_initializer(&initializer)?;
    let layout = sdk::pin(&program, SessionInput::Session(&session))?;
    fs::write("seed-layout.json", serde_json::to_vec_pretty(&layout)?)?;
    Ok(())
}
```

## Recover and investigate code

These are independent recipes for investigation or deliberate intermediate
artifact reuse. They are not additional required stages in the header workflow.

### Deobfuscate and optionally save a compiled kernel

`deob` returns `.code`; kernel compilation is requested explicitly with
`artifact: true`. Omit that flag when you only want readable source.

```rust
use shape::sdk::{self, DeobOptions};
use std::{error::Error, fs};

fn main() -> Result<(), Box<dyn Error>> {
    let raw_seed = fs::read_to_string("seed.js")?;
    let result = sdk::deob(&raw_seed, DeobOptions { artifact: true })?;
    fs::write("seed.deob.js", result.code)?;
    let kernel = result.artifact.ok_or("requested kernel artifact missing")?;
    fs::write("seed.kernel.json", sdk::kernel::to_json(&kernel)?)?;
    Ok(())
}
```

If deobfuscated text already exists, `sdk::report(KernelSource::Deobfuscated(
&source))` compiles it into a kernel. `KernelSource::Raw` accepts raw seed text.
A separate report run is unnecessary when `deob` already returned the kernel.
The kernel returned by `deob(..., artifact: true)` retains the raw source
identity. A kernel compiled from `KernelSource::Deobfuscated` instead identifies
that deobfuscated text; it cannot be paired with the original raw bytes in
`program_with_artifact` or prepopulate a cache entry for those raw bytes.

### Reuse a kernel for whole-program tracing

This program consumes `seed.kernel.json` from the preceding recipe and the
original matching `seed.js`. It avoids recompiling the kernel while tracing
and cleaning the program. It does not need a disk-cache policy to reuse an
explicit loaded artifact.

```rust
use shape::sdk::{self, CachePolicy, ProgramOptions};
use std::{error::Error, fs};

fn main() -> Result<(), Box<dyn Error>> {
    let raw_seed = fs::read_to_string("seed.js")?;
    let kernel = sdk::kernel::load(&fs::read_to_string("seed.kernel.json")?)?;
    let traced = sdk::trace::program_with_artifact(
        &raw_seed,
        &kernel,
        ProgramOptions {
            cache: CachePolicy::Disabled,
            thread_cleanup: true,
            no_cleanup: false,
            jobs: None,
            ..Default::default()
        },
    )?;
    if !traced.complete {
        return Err(format!("incomplete trace: {:?}", traced.stopped).into());
    }
    fs::write("program.js", traced.code)?;
    Ok(())
}
```

The source identity must match the compiled kernel; the overload still uses
raw source during program processing. A kernel alone is not a complete saved
program. `jobs: None` selects available cores, while `Some(n)` caps concurrency.

### Inspect a kernel and its operations

Source-based projections below read raw `seed.js`. They write focused kernel,
operation-catalog and single-operation documents. Pass a real operation ID
from your seed's catalog as the program's argument; IDs are build-specific.

```rust
use shape::sdk::{self, KernelShowOptions, KernelSource};
use std::{env, error::Error, fs};

fn main() -> Result<(), Box<dyn Error>> {
    let op: u32 = env::args()
        .nth(1)
        .ok_or("pass an operation ID from this seed")?
        .parse()?;
    let raw_seed = fs::read_to_string("seed.js")?;
    let kernel = sdk::kernel::show(KernelSource::Raw(&raw_seed), KernelShowOptions::default())?;
    fs::write("kernel-view.json", serde_json::to_vec_pretty(&kernel)?)?;
    let catalog = sdk::ops::list(&raw_seed)?;
    fs::write("operations.json", serde_json::to_vec_pretty(&catalog)?)?;
    let operation = sdk::ops::show(&raw_seed, op)?;
    fs::write("operation.json", serde_json::to_vec_pretty(&operation)?)?;
    Ok(())
}
```

`KernelShowOptions.operations` filters operation projections. Loaded kernels
have direct getters such as `templates()` for compiled runtime inspection,
but those templates are not the full source-derived operations catalog.
These source views do not silently fall back to invented facts from a kernel.

### Trace a single thread

Use a PC/state pair obtained from the same seed's descriptors or investigation,
not arbitrary numbers copied from another build. This program requires both
as command-line arguments and uses an already compiled kernel. It checks both
completion and nullable output before saving a usable thread.

```rust
use shape::sdk::{self, KernelInput, ThreadOptions};
use std::{env, error::Error, fs};

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let pc = args.next().ok_or("pass this seed's thread PC")?.parse()?;
    let state = args
        .next()
        .ok_or("pass this seed's thread state")?
        .parse()?;
    let kernel = sdk::kernel::load(&fs::read_to_string("seed.kernel.json")?)?;
    let traced = sdk::trace::thread(
        KernelInput::Artifact(&kernel),
        ThreadOptions::new(pc, state),
    )?;
    if !traced.complete {
        return Err(format!("thread stopped: {}", traced.stop).into());
    }
    let code = traced.code.ok_or("thread produced no code")?;
    fs::write("thread.js", code)?;
    Ok(())
}
```

`KernelInput::Source(&raw_seed)` is the alternative when starting from raw
source; it compiles an ephemeral kernel. `ThreadOptions` supports descriptor,
initial slots, input snapshots, branch overrides, step limit and self-checks.
Default steps are one million and outlining is enabled. `partial: true` can
retain incomplete code for investigation; it does not make the trace complete.

### Clean one emitted thread

This is local cleanup of `thread.js`, not semantic naming and not a required
extra pass over the complete program. Cleanup returns `.code` and detailed
counters; naming recipes below return `.source` instead.

```rust
use shape::sdk;
use std::{error::Error, fs};

fn main() -> Result<(), Box<dyn Error>> {
    let thread = fs::read_to_string("thread.js")?;
    let cleaned = sdk::cleanup::thread(&thread)?;
    println!(
        "{} cleanup rounds; {} helpers remain",
        cleaned.rounds, cleaned.remaining_helpers
    );
    fs::write("thread.cleaned.js", cleaned.code)?;
    Ok(())
}
```

### Identify roles, plan names and rename deliberately

Naming investigates emitted program text and takes extra analysis time. This
independent program saves a plan/inventory before applying explicit renaming.
Thread naming and collector naming operate separately on the original program;
outputs are labeled separately rather than implicitly chained.

```rust
use shape::sdk::{self, NamingCollectorsOptions, NamingThreadsOptions};
use std::{error::Error, fs};

fn main() -> Result<(), Box<dyn Error>> {
    let program = fs::read_to_string("program.js")?;
    let plan = sdk::naming::plan(&program)?;
    fs::write("naming-plan.json", serde_json::to_vec_pretty(&plan)?)?;
    let inventory = sdk::naming::inventory(&program)?;
    fs::write(
        "naming-inventory.json",
        serde_json::to_vec_pretty(&inventory)?,
    )?;
    let threads = sdk::naming::threads(
        &program,
        NamingThreadsOptions {
            rename: true,
            ..Default::default()
        },
    )?;
    fs::write("program.threads.js", threads.source)?;
    fs::write(
        "threads.naming.json",
        serde_json::to_vec_pretty(&threads.report)?,
    )?;
    let collectors = sdk::naming::collectors(&program, NamingCollectorsOptions { rename: true })?;
    fs::write("program.collectors.js", collectors.source)?;
    fs::write(
        "collectors.naming.json",
        serde_json::to_vec_pretty(&collectors.report)?,
    )?;
    Ok(())
}
```

Both options default to `rename: false` for identification without renaming.
Thread naming also accepts a `LabelReference`. Naming plans and inventories
are useful reports, not cached compiler state or prerequisites for decoding.

## Advanced capabilities

These named Rust capabilities expose public engine building blocks beyond the
shared JSON command set. They do not add JSON command IDs or change other SDKs.

| Task | Start here |
| --- | --- |
| Compare operation kinds | [Classify two captures](#classify-operations-across-two-captures) |
| Compare reachable functions | [Walk and fingerprint](#walk-reachable-functions-and-compare-fingerprints) |
| Compare semantic naming | [Analyze program revisions](#compare-naming-across-program-revisions) |
| Inspect or acquire seeds | [Parse and import a seed](#parse-and-import-a-seed) |
| Handle raw report channels | [Frame headers](#frame-a-raw-report-as-headers) or [payload bytes](#frame-payload-bytes-without-headers) |
| Serialize generic values | [Pack and unpack](#pack-and-unpack-generic-values) |

### Classify operations across two captures

Provide `seed.js` and `seed-other.js` from two builds you want to compare. The
summary groups equivalent operation kinds across inputs; it is not a claim
that operation IDs remain identical between builds.

```rust
use shape::sdk;
use std::{error::Error, fs};

fn main() -> Result<(), Box<dyn Error>> {
    let first = sdk::ops::classify::source(&fs::read_to_string("seed.js")?)?;
    let second = sdk::ops::classify::source(&fs::read_to_string("seed-other.js")?)?;
    let kinds = sdk::ops::classify::seeds(&[first, second])?;
    for kind in &kinds {
        println!("{}: {} handler occurrences", kind.kind, kind.handlers.len());
    }
    // Classification domain types are typed inspection results. Choose an
    // explicit output document when persisting them.
    let document: Vec<_> = kinds
        .iter()
        .map(|kind| serde_json::json!({
            "kind": kind.kind,
            "nature": kind.nature.as_str(),
            "scope": kind.scope.as_str(),
            "handlers": kind.handlers,
        }))
        .collect();
    fs::write("operation-kinds.json", serde_json::to_vec_pretty(&document)?)?;
    Ok(())
}
```

### Walk reachable functions and compare fingerprints

This program loads the compiled kernel, walks reachable functions, and saves
fingerprints under stable function keys. Pass a previous `corpus-current.json`
filename as an optional argument to compare against a baseline for the same
logical seed name. With no argument, it only produces the current document.

```rust
use shape::sdk::{self, trace::corpus::ReachableOptions};
use std::{env, error::Error, fs};

fn main() -> Result<(), Box<dyn Error>> {
    let kernel = sdk::kernel::load(&fs::read_to_string("seed.kernel.json")?)?;
    let reached = sdk::trace::corpus::reachable(
        &kernel,
        ReachableOptions {
            jobs: std::thread::available_parallelism()?.get(),
            cleanup: true,
            ..Default::default()
        },
    )?;
    let functions: serde_json::Map<String, serde_json::Value> = reached
        .into_iter()
        .map(|(function, result)| (function.key(), result.print))
        .collect();
    let current = serde_json::json!({ "seed.js": functions });
    if let Some(path) = env::args_os().nth(1) {
        let baseline: serde_json::Value = serde_json::from_slice(&fs::read(path)?)?;
        let comparison = sdk::trace::corpus::compare(&baseline, &current)?;
        println!(
            "{} regressions; {} changes",
            comparison.regressions.len(),
            comparison.changes.len()
        );
        for regression in comparison.regressions {
            eprintln!("{regression}");
        }
    }
    fs::write("corpus-current.json", serde_json::to_vec_pretty(&current)?)?;
    Ok(())
}
```

Fingerprints retain completion/stop facts. Inspect them before accepting a
reachable walk; not every function necessarily completed. `compare_inline`
compares outlined/inline fingerprints; `fingerprint` accepts a public prefix
trace and its parameters.

Choose the corpus entry point for your input:

- **Seed file, full convergence:** `trace::corpus::walk_seed(&seed_path,
  &options, CachePolicy::Disabled)` with `trace::corpus::CorpusOptions` runs
  requested self-checks and branch output. The explicit cache policy overrides
  `options.report_dir`.
- **Raw source in memory:** `trace::corpus::load` and `source` take a seed name,
  supplied source bytes and explicit `CachePolicy`. The name does not load a file.
- **Loaded kernel:** `reachable` works in memory without a disk cache. These
  primitives do not run the full convergence/self-check workflow.

Baseline file comparison and publication remain your application's choice.

### Compare naming across program revisions

Read two complete emitted programs. Give each the same logical name to compare
roles across revisions; names within one corpus must be nonempty and unique.

```rust
use shape::sdk::{self, naming::corpus::Source};
use std::{error::Error, fs};

fn main() -> Result<(), Box<dyn Error>> {
    let baseline = sdk::naming::corpus::analyze(&[Source {
        name: "seed.js".into(),
        source: fs::read_to_string("program.baseline.js")?,
    }])?;
    let current = sdk::naming::corpus::analyze(&[Source {
        name: "seed.js".into(),
        source: fs::read_to_string("program.js")?,
    }])?;
    let differences = sdk::naming::corpus::compare(&baseline, &current)?;
    for difference in differences {
        println!(
            "{}: {} missing roles; {} added roles; {} name differences",
            difference.name,
            difference.missing_roles.len(),
            difference.added_roles.len(),
            difference.name_divergences.len()
        );
    }
    Ok(())
}
```

Analysis returns a typed plan and rename result per input. It does not scan a
directory, reuse a cached AST or publish a CLI vendor/manifest matrix.

### Parse and import a seed

This independent recipe accepts the captured kernel URL as an argument. It
classifies local `seed.js`, extracts the URL's `seed` token, then explicitly
imports the validated kernel into `seed-store`. Use the actual matching URL;
`https://example.test/kernel.js?seed=synthetic` only illustrates URL shape.

```rust
use shape::sdk::{self, seeds::Script};
use std::{env, error::Error, fs, path::Path};

fn main() -> Result<(), Box<dyn Error>> {
    let reference = env::args()
        .nth(1)
        .ok_or("pass the matching captured kernel URL")?;
    let bytes = fs::read("seed.js")?;
    match sdk::seeds::parse(&bytes)? {
        Script::Kernel(summary) => println!("kernel: {} bytes", summary.bytes),
        Script::Launcher(_) => {
            return Err("seed.js must contain the fetched kernel, not a launcher".into())
        }
    }
    let token = sdk::seeds::token(&reference, "seed")?;
    let stored = sdk::seeds::import(Path::new("seed-store"), &token, None, &bytes)?;
    println!("saved kernel at {}", stored.kernel_path.display());
    Ok(())
}
```

Choose seed operations by where the input comes from:

- **Parsing:** `parse_launcher` extracts a kernel reference; `resolve_url`
  resolves it against your base URL. `sites`/`site` inspect the registry.
- **Network:** explicitly call `fetch` or `fetch_direct` with a client from
  `seeds::client(ClientConfig)`, its `Arc`, a URL and `FetchOptions`. Configure
  request/connect timeouts, response size and redirect limits on the client.
- **Storage:** `import` validates syntax/structure and writes to your chosen
  directory. Supply the matching source/token association from the capture;
  a token alone does not prove identity.

Fetching returns bytes without saving files.

### Frame a raw report as headers

Named-field decode/encode remains the normal capture workflow. Lower-level
header framing is useful when handling raw report channels. This recipe derives
the header profile from the executable artifact, decodes a capture once to
obtain its retained raw report, then frames that report without interpreting
its named fields again. Output is a full-name `Headers` map, distinct from the
suffix-keyed `CaptureHeaders` document.

```rust
use shape::sdk::{self, CaptureHeaders, DecodeOptions};
use std::{error::Error, fs};

fn main() -> Result<(), Box<dyn Error>> {
    let artifact = sdk::artifact::load(&fs::read("decode-artifact.json")?)?;
    let capture: CaptureHeaders = serde_json::from_slice(&fs::read("headers.json")?)?;
    let decoded = sdk::decode(&artifact, &capture, None, DecodeOptions::default())?;
    let layout = &artifact.artifact().layout;
    let codec = sdk::headers::codec(layout.profile())?;
    let headers = sdk::headers::encode(&codec, &decoded.metadata.original_report)?;
    let report = sdk::headers::decode(
        &codec,
        &headers,
        layout.report.positional_signals.len(),
        layout.keysets.clone(),
    )?;
    println!(
        "{} positional slots framed",
        report.positional_signals.len()
    );
    fs::write("headers.framed.json", serde_json::to_vec_pretty(&headers)?)?;
    Ok(())
}
```

`encode_bytes`/`decode_bytes` operate on opaque `Payload` values;
`encode_finalized`/`decode_finalized` handle finalized payload text and sibling
header context. Report decode requires the source-derived positional count
and ordered keysets: they cannot be inferred from arbitrary ciphertext.

### Frame payload bytes without headers

This synthetic byte round trip demonstrates the payload layer only. It uses
real profile geometry from your artifact, explicitly supplied seeds, and test
bytes rather than pretending they are a captured signal. Successful byte
decryption establishes neither authenticity nor signal meaning.

```rust
use shape::sdk::{self, payload::PayloadProfile};
use std::{error::Error, fs};

fn main() -> Result<(), Box<dyn Error>> {
    let artifact = sdk::artifact::load(&fs::read("decode-artifact.json")?)?;
    let profile = PayloadProfile::from(artifact.artifact().layout.profile());
    let codec = sdk::payload::codec(profile)?;
    let plaintext = b"synthetic framing sample";
    let text = sdk::payload::encode_bytes(&codec, plaintext, [1, 2])?;
    let recovered = sdk::payload::decode_bytes(&codec, &text)?;
    assert_eq!(recovered.plaintext()?, plaintext.to_vec());
    fs::write("payload.synthetic.txt", text)?;
    Ok(())
}
```

`payload::encode`/`decode` instead pack/unpack a generic channel `Report` with
positional count/keysets. These operations do not collect or interpret signals.

### Pack and unpack generic values

Superpack runs independently of source analysis and encryption. Use its typed
values to preserve distinctions such as undefined, binary, dates and IEEE-754
number bits. This public synthetic example packs a small array and checks the
canonical decoded value.

```rust
use shape::sdk::{self, PackValue, SerializerOptions};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let value = PackValue::Array(vec![
        PackValue::Bool(true),
        PackValue::String("sample".into()),
    ]);
    let options = SerializerOptions::default();
    let bytes = sdk::pack(&value, &options)?;
    let recovered = sdk::unpack_canonical(&bytes, &options)?;
    assert_eq!(recovered, value);
    println!("{} packed bytes", bytes.len());
    Ok(())
}
```

`unpack` retains the richer `PackDecoded` result. Options govern omitted
keysets, depth handling, extensions and resource limits; they must match the
encoding. The supported source-derived subset is described in the
[superpack reference](../../docs/generator/superpack.md).

## Install and build

The SDK is the **root `shape` crate** in this checkout. `sdk/rust` contains this
guide, not a separate package. No registry release is published.

```toml
[dependencies]
shape = { path = "/path/to/repository" }
serde_json = "1"
```

Use Rust 1.92 or newer, matching `Cargo.toml`. The crate is `publish = false`
and has no optional feature switches. Rust callers do not need
`scripts/build-sdk.sh` or the C ABI. Use `cargo run --release -- ...` in your
consumer crate for expensive seed tracing/recovery. The library and CLI share
the same crate version; build the CLI from the repository root when needed:

```sh
cargo build --release --locked --bin shape
export PATH="$PWD/target/release:$PATH"
shape --help
```

Each Rust block above is an independent `main.rs` with imports and error
handling. Install the two dependencies once in your consumer crate, choose a
recipe, and provide its stated files. No repository `examples/` folder is
required. `serde_json` handles genuine input/output documents, heterogeneous
report fields and edited JSON values; SDK operation selection uses Rust
functions and enums.

## API

### Named operations

All fallible operations return `sdk::Result<T>` with the same SDK stack/panic
boundary. `version` and `commands` are infallible. Borrowed arguments need not
be `'static`. The primary namespaces mirror the CLI's conceptual operations:

| Operation | Rust function/input → result |
| --- | --- |
| Version/discovery | `version() → VersionInfo`, `commands() → &'static [&'static str]` |
| Deobfuscate | `deob(&str, DeobOptions) → DeobResult { code, artifact }` |
| Compile kernel report | `report(KernelSource) → KernelArtifact` |
| Inspect kernel | `kernel::show(KernelSource, KernelShowOptions) → Value` |
| List/show operations | `ops::list(&str)`, `ops::show(&str, u32) → Value` |
| Trace program | `trace::program(&str, ProgramOptions) → ProgramTrace` |
| Trace thread | `trace::thread(KernelInput, ThreadOptions) → ThreadTrace` |
| Clean emitted thread | `cleanup::thread(&str) → ThreadCleanup` |
| Name threads | `naming::threads(&str, NamingThreadsOptions) → NamedThreads` |
| Name collectors | `naming::collectors(&str, NamingCollectorsOptions) → FocusedCollectors` |
| Plan names | `naming::plan(&str) → SemanticNamingPlan` |
| Inventory names | `naming::inventory(&str) → NamingInventory` |
| Detailed signal schema | `signals::schema(&str, SchemaOptions) → SignalSchemaReport` |
| Program properties | `signals::show(&str) → ProgramProperties` |
| Program inputs | `signals::inputs(&str) → ProgramInputContextReport` |
| Signal contracts | `signals::contracts(&str) → SignalContractReport` |
| Pin layout | `pin(&str, SessionInput) → SeedLayout` |
| Parse session | `session::from_initializer(&str) → Session` |
| Compile executable artifact | `artifact::compile(ProgramInput, &Session, &str, ArtifactOptions) → LoadedArtifact` |
| Export document schema | `artifact::compact_schema(&DecodeArtifact) → CompactSeedSchema` |
| Decode | `decode(&LoadedArtifact, &CaptureHeaders, Option<&Session>, DecodeOptions) → DecodedPayload` |
| Explain one signal | `explain(&LoadedArtifact, &DecodedPayload, &str, Option<&Session>, bool) → NamedSignal` |
| Encode full payload | `encode(&LoadedArtifact, &DecodedPayload, Option<&Session>) → CaptureHeaders` |
| Encode bare named fields | `encode_signals(&LoadedArtifact, &NamedSignals, Option<&Session>) → CaptureHeaders` |
| Export loaded schema | `schema(&LoadedArtifact) → CompactSeedSchema` |

The first 21 shared stateless IDs map to these named operations; the loaded
operations and Rust additions are not extra entries in `COMMANDS`.
[Advanced capabilities](#advanced-capabilities) lists classification, corpus,
acquisition and framing additions.

### Load, reuse and persist

| Function | Purpose |
| --- | --- |
| `artifact::load(&[u8])` | Parse/validate executable decode JSON once into a `LoadedArtifact` |
| `artifact::from_artifact(DecodeArtifact)` | Validate an owned typed document without a serialization round trip |
| `loaded.artifact()` | Immutable borrow of the underlying decode document |
| `loaded.to_json()` | Explicit executable-artifact serialization, returning `Vec<u8>` |
| `kernel::load(&str)`, `kernel::to_json(&KernelArtifact)` | Canonical compiled-kernel loading/serialization |
| `trace::program_with_artifact(&str, &KernelArtifact, ProgramOptions)` | Trace matching raw source with an explicit compiled kernel |
| `signals::catalog()`, `signals::describe(&str)` | Static catalog and one canonical card |
| `signals::compact_schema(&str, CompileOptions)` | Descriptive export from traced source without a session |

### Important defaults

| Options | Named Rust defaults |
| --- | --- |
| `ProgramOptions` | Cache disabled; thread cleanup enabled; full mechanical finishing enabled (`no_cleanup: false`); one million steps; available cores (`jobs: None`); quiet |
| `ArtifactOptions` | Cache disabled; strict (`direct: false`); little-endian float bytes; raw input uses the full cleanup policy |
| `DeobOptions` | No compiled kernel unless `artifact: true` |
| `ThreadOptions::new(pc, state)` | One million steps; outlining enabled; no partial output or self-check by default |
| Naming options | Identify roles; no renaming by default |
| `SchemaOptions` | Full analysis with finder roles; optional seed name |
| `DecodeOptions` | No eager per-signal metadata or exact verification |

### Typed library APIs

Lower-level modules remain public for integrations requiring engine-specific
control: `shape::trace`, `shape::naming`, `shape::signals::{analysis, extract,
decode, payload, wire}`, and `shape::KernelArtifact`. Prefer the named facade
for common tasks: it adds a consistent SDK error and large-stack boundary.
Direct engine calls retain their module-specific errors and stack requirements.
In particular, mutable `DecodeArtifact::decode` validates on each call;
`LoadedArtifact` is the immutable repeated-use interface.

## Precision

Keep artifact bytes intact when loading and preserve typed payloads or
`serde_json::Value` values at document boundaries. The crate enables JSON
arbitrary precision. Never route u64 tokens, IEEE-754 bit patterns or packed
integers through `f64`; that can lose information. `PackValue::Number(Number)`
records number bits explicitly, including negative zero; do not substitute a
plain JSON float for its typed representation.

## Errors

Named operations return `shape::sdk::Error` with public `code` and `message`
fields and a standard `Display`/`Error` implementation. The complete programs
above use `Box<dyn Error>` to combine SDK, I/O, JSON and argument errors through
`?`. Applications can instead match `error.code` at their boundary.

| Code | Action |
| --- | --- |
| `invalid_request` | Correct the request, options, source identity or canonical signal name |
| `unknown_command` | Check the legacy command or generator method name |
| `failed` | Read the diagnostic; check input format, matching artifact/session and supported encoding |
| `panic` | An engine panic was caught; retain a diagnostic for investigation |

Artifact loading maps malformed/invalid bytes to `failed`, matching the legacy
loader. Messages are diagnostic, not a stable matching interface. Typed input
enums eliminate some invalid combinations before execution. SDK panic handling
cannot catch process aborts or failure to create a worker thread.

## Threading and lifetime

`LoadedArtifact` is `Clone + Send + Sync`. Clones share immutable validated
state through an `Arc`; dropping one does not invalidate another. No `close`
is necessary. Calls take explicit artifacts and may use the same handle from
multiple threads. Borrowing `.artifact()` performs no processing or isolation.
Keep owned payloads for each edit instead of mutating shared data.

### Large stacks

Named processing operations, legacy dispatch and generator operations run
engine work on a large-stack worker (`shape::LARGE_STACK_BYTES`, 256 MiB
reserved, not committed) with panic-to-error conversion. Input file I/O and
caller JSON conversion run on your thread. Calls are synchronous; use your
application's blocking worker facility when integrating with an async runtime.

Direct engine functions do not create that worker. When bypassing the SDK,
wrap deep AST/tracing work in `shape::run_with_large_stack`, moving owned inputs
into the closure and handling the engine's return type. Running direct engine
work in one such closure can amortize worker creation across a batch. A named
SDK call still creates its own worker; wrapping several named calls in another
large-stack closure does not turn them into a persistent worker session.

## Performance

Use optimized/release builds for expensive seed recovery. Trace once, compile
once, then keep the loaded decode artifact outside capture loops. Decode/
encode/explain use its in-memory validated state; no JSON save/load round trip
is needed between compilation and use.

The choices control different costs: kernel caching avoids kernel compilation;
a saved program avoids retracing; `LoadedArtifact` avoids repeated document
loading/validation and recipe setup. Source-only reports still parse/analyze.
No persistent trace, naming or AST cache is promised. Bound tracing by `steps`
and select `jobs` for the resources available to your application.

Keep full mechanical cleanup enabled for raw-seed recovery. Semantic naming
operations are optional and separate from mechanical normalization. Request eager
metadata, exact verification, optional naming and extra report exports only
when their output is useful. These are cost boundaries, not benchmark claims.

## Troubleshooting

| Symptom | Check |
| --- | --- |
| Toolchain rejected | Rust must meet the root crate's 1.92 MSRV |
| Artifact load fails | Supply executable decode JSON, not a kernel artifact or schema |
| Site ID mismatch | Use the original matching site/build/session inputs |
| Encoding an edit fails | Keep the full original report and choose a supported field/value |
| Signal missing | Check complete trace, finder diagnostics and whether this build captures that signal |
| Incomplete trace or missing thread code | Inspect stopped/stop facts and the step limit; do not compile partial investigation output as complete |
| Explicit cache load fails | Verify source identity, filename and canonical kernel format; stale explicit entries are errors |
| Lost integer precision | Remove conversions through floating point |
| Slow seed recovery | Use optimized builds, full cleanup defaults, and the appropriate saved artifact/reuse boundary |
| Stack overflow in direct engine code | Use the SDK facade or `run_with_large_stack` |

## Tests

The repository's focused Rust integration tests cover SDK operation parity,
loaded-artifact round trips, explicit cache policies and extension primitives.
The [shared contract harness](../tests/README.md) checks the existing binding
contract across runtimes; see [test documentation](../../tests/README.md) for
fixture ownership. README recipes require their declared matching input files
for execution. Compilation alone checks types, not successful decoding of an
arbitrary build. No expensive seed generation is needed to type-check them.
