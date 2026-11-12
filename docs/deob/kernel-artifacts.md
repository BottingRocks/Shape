# Compiled kernel artifacts

A `KernelArtifact` preserves an admitted prefix preparation for later tracing
without the original JavaScript or another Oxc analysis. It is the file written
by `shape report` and `shape deob --report`; inspection bundles and serialized
pipeline reports are different formats.

Compilation establishes source relationships. Loading checks format, bounds,
integrity, and structural consistency, then reconstructs execution indexes.
Integrity is not source authenticity. See [trace input modes](../tracing/trace-cli.md#input-modes)
for command usage and the [prefix module guide](../../src/dispatch/prefix/README.md)
for the prepare → bind → invoke API.

## The artifact, field by field

`KernelArtifact` is one JSON envelope whose `payload` is a base64 string. Inside
that string is zstd-compressed JSON, not an opaque binary format. The choice is
deliberate: every structural check that applied to the in-memory report still
applies after decompression, and `deny_unknown_fields` keeps a corrupted or
hand-edited payload from being silently accepted.

~~~text
{
  "format": "shape-kernel-artifact/v1",
  "compiler_provenance": "shape-rust-oxc-0.1",
  "source_hash": "<16 lowercase hex>",
  "artifact_source_hash": "<16 lowercase hex>",
  "source_basis": "raw_source_observed" | "declared_only",
  "standardized_length": <u64, 1..=u32::MAX>,
  "integrity_digest": "<64 lowercase hex>",
  "payload": "<base64(zstd(json))>"
}
~~~

| Envelope field | Why it exists |
| --- | --- |
| `format` | Version gate; only `shape-kernel-artifact/v1` is accepted |
| `compiler_provenance` | Compiler gate; `shape-rust-oxc-0.1` is the only accepted writer |
| `source_hash` | The declared capture identity (FNV-1a 64) |
| `artifact_source_hash` | Identity of the standardized report artifact that was re-proved |
| `source_basis` | Whether raw bytes were observed or only declared |
| `standardized_length` | Bound for every source span in the payload |
| `integrity_digest` | SHA-256 over the reconstruction material, recomputed on load |
| `payload` | The whole runtime, compressed |

The payload's fields are the preparation, minus what cannot cross a wire:

| Payload field | Content |
| --- | --- |
| `bytecode` | The initial image, base64 |
| `dispatch_cells` | `state -> opcode -> { next_state, selector }` |
| `templates` | Sanitized `HandlerTemplateReport` |
| `operands` | Operand handlers with descriptors and spans |
| `patch_selector`, `patch_span`, `patch_operand_width` | The proven write handler |
| `bytecode_span`, `run_loop_span` | Proven source spans |
| `completion_register`, `exception_stack_field`, `exception_helper` | Completion protocol facts |
| `constant_tables` | Name plus entries for each proven string table |
| `limits` | Compile-time `PrefixLimits` |
| `default_entry` | The entry found in the capture, if any |
| `entry_initial_slots` | Slots that entry initializes |
| `descriptor_records` | Every descriptor row, including typed-invalid ones |
| `initial_slot_inputs` | Slot to value mapping for the entry |
| `string_fold_plan` | The compiled VM-string fold plan |
| `wasm` | Present only for a WASM kernel |

`thread_descriptors` is not on the wire: it is reconstructed on load from the
valid rows of `descriptor_records`. That is why a single projection rule
("valid records == descriptors") can be validated, and why a hand-injected
descriptor list fails.

### Compile paths

| Entry point | Input | `source_basis` | Notes |
| --- | --- | --- | --- |
| `compile` | source + report + limits | `raw_source_observed` | Alias of `compile_from_raw` |
| `compile_from_raw` | source + report + limits | `raw_source_observed` | The only path with raw bytes in hand |
| `compile_from_report` | report + limits | `declared_only` | Alias of the next row |
| `compile_from_existing_pipeline_report` | report + limits | `declared_only` | Uses `prepare_prefix_from_saved_report` |
| `compile_source` | source + limits | `raw_source_observed` | Runs `deobfuscate_with_report`, then `compile_from_raw` |
| `compile_from_deobfuscated` | standardized source + limits | `declared_only` | Rebuilds the standardized projection; recovers the operation table if the projection lacks it |
| `from_preparation` | a `PrefixPreparation` | `declared_only` | The export path; sanitizes and prebuilds, with no raw facts |

The raw paths additionally `install_actual_facts(report)`, which extracts the
entry invocation and every thread-descriptor row from the report's actual
properties, and compile the `VmStringFoldPlan`. The saved-report and
deobfuscated paths do the same, but they cannot claim raw capture authenticity.

`compile_from_deobfuscated` does **not** replay clean/rename. It rebuilds the
standardized report projection, re-states canonical helper roles and
context/memory layout fields, and, if `proven_operation_tables` is empty,
recovers the single table declaration structurally. That is enough for the
payload to match a raw compile of the same standardized output; the regression
test `deobfuscated_compile_matches_the_raw_runtime_payload` compares every
payload field to prove it.

### Bounds, checked before allocation

| Bound | Value | Where |
| --- | --- | --- |
| Envelope JSON | 64 MiB (`MAX_ARTIFACT_BYTES`) | `from_json`, before parsing |
| Compressed payload | 8 MiB | base64 decode with a limit |
| Decompressed payload JSON | 32 MiB | streaming read loop |
| zstd window log | 23 | `decoder.window_log_max` before reading |
| Image | 8 MiB | `validate`, and `prepare_with_limits` |
| Dispatch cells | 1 000 000 | `validate` |
| Handlers | 100 000 per report | `validate` |
| Descriptor rows | 100 000 | `validate` |
| Descriptor slots | 1 000 000 per descriptor | `validate` |

The window bound matters for security, not just size: a small compressed payload
can declare a huge window and force a large allocation if the decoder is allowed
to honour it. `MAX_ZSTD_WINDOW_LOG` is set before any read, so such a payload
fails before it can expand.

### What `validate` checks

Grouped by what a failure means:

| Group | Checks |
| --- | --- |
| header | exact `format`; `source_basis` is one of the two accepted strings; `standardized_length` is nonzero and at most `u32::MAX`; both hashes are exactly 16 lowercase hex characters |
| image and table | image within bounds; cells within bounds; no empty row; every cell's `next_state` is a real state |
| identity | template and operand tables are the same `ProvenOperationTable`; handler counts within bounds; `patch_operand_width == 4` |
| descriptors | each record has exactly one of `descriptor` or `invalid`; the valid projection equals the descriptor list; row, slot and entry bounds |
| recipes | every executable template passes `admit_recipe` with default `RecipeBounds`; no unsupported fragment carries source; every step span is inside `standardized_length`; no `Unknown` source; no `FunctionLiteral` value survives |
| closure plans | depth at most 256; at most 32 parameters and 64 statements; valid and unique parameter names; every referenced parameter is declared; every external name is a valid identifier |
| spans | patch, bytecode, run loop, exception helper, handler, operand and captured-dependency spans are all inside `standardized_length` |
| entry | `default_entry.pc < image.len()` and `default_entry.state < table.len()` |

The recipe and closure-plan checks are what make "source-free" enforceable
rather than aspirational. If a source fragment survived sanitization, the
artifact would fail its own validation; it cannot be written, let alone loaded.

### The integrity digest

The digest is SHA-256 over the serialization of exactly these fields:

~~~text
format, compiler_provenance, source_hash, artifact_source_hash,
source_basis, standardized_length, payload (the canonical JSON string)
~~~

The payload is re-serialized from the parsed wire struct before hashing, so the
digest is independent of the compressed bytes and of key order. Recompressing a
valid artifact at a different level produces the same digest and loads
successfully; changing any field of the envelope or any byte of the payload
changes it. The tracked tests pin both directions: a mutated `source_hash`
fails with `IntegrityMismatch`, and a recompressed payload still loads.

The digest is an integrity check, not source authenticity. It proves the
artifact is internally consistent with the identity it declares; it does not
prove the declared raw capture ever existed. `verify_source_identity(source)` is
the separate, explicit identity check a caller may run when raw bytes are
available.

### Sanitization and the closure plan

`from_preparation` walks every template and:

* clears the `source` of each unsupported fragment;
* clears `RecipeOperation::Unknown { source }`;
* replaces every `SemanticValue::FunctionLiteral { source }` with a
  `ClosurePlan`, compiled by `compile_closure_plan`, or fails the compile.

A closure plan is a tiny bounded program: a parameter list and a body of
`CharCodeAt`, `AddOne`, `NullIndexCall` or `External` expressions, in expression
or return statements. The emitter later renders it as the function literal the
capture had. The supported subset is narrow on purpose: `function (x) { return x
+ 1; }` and `function (sourceString) { return sourceString.charCodeAt(); }` are
supported; `eval`, `x + 2`, a member read, `async`, a generator, a named
function, `"use strict"`, and a body with an ASI-sensitive newline are not.

### Preparing and instantiating

| Call | Behavior |
| --- | --- |
| `prepare` | `validate`, then return the prebuilt preparation if present, else build one with the compile-time limits |
| `prepare_with_limits` | `validate_runtime_limits`, build, replace the preparation's limits |
| `instantiate` | Use the prebuilt preparation (an artifact always has one after load or export) and call `thread()` |

These calls form a library integration fragment: `artifact_path` is a
`&std::path::Path`, and the enclosing function returns
`Result<(), Box<dyn std::error::Error>>`. The descriptor is illustrative; a real
invocation must match the entry's recorded initial and argument slots. The empty
argument list still writes `SymbolicValue::Undefined` into slot 0 because startup
fills every missing positional argument, so this example shows the startup shape
rather than a concrete capture invocation.

```rust
use shape::{
    DispatchStart, KernelArtifact, PrefixLimits, SymbolicValue, ThreadDescriptor,
    ThreadInputs,
};

let json = std::fs::read_to_string(artifact_path)?;
let artifact = KernelArtifact::from_json(&json)?;

// prepare() keeps the limits recorded at compile time; use
// prepare_with_limits() to apply the runtime policy the caller asked for.
let prepared = artifact.prepare_with_limits(PrefixLimits::default())?;

let entry = artifact.entry_facts().ok_or("artifact has no default entry")?;
let descriptor = ThreadDescriptor::new()
    .initial_slots([0])
    .argument_slots([0]);
let thread = prepared.thread(
    DispatchStart { pc: entry.pc as usize, state: entry.state as usize },
    descriptor,
    ThreadInputs::new(),
)?;
let report = thread.invoke(std::iter::empty::<SymbolicValue>())?;
```

`validate_runtime_limits` re-checks the artifact's dimensions against the
caller's new limits before rebinding them: image size, cell count, every cell's
integer fields, and the nonzero requirements for `max_dispatch_steps` and
`max_integer`. It does not re-run source proofs; there are none to run. That is
why a loaded artifact can be walked with a stricter runtime policy without
becoming a different program.

### The string-fold plan

`VmStringFoldPlan` is compiled from the report at compile time and validated
before it is written. It travels inside the artifact so a consumer can call
`fold_emitted_vm_strings(emitted)` and remove proven VM string decoding from the
emitted JavaScript. An artifact without a plan (or one whose plan declines a
particular program) returns the emitted JavaScript unchanged and records a
`VmStringFoldReport` decline reason such as
`HelperOrDecoderNotProven`. Folding is an emission optimization, never a
correctness claim about the trace.

### Lifecycle, end to end

~~~text
source + report
   │  compile_from_raw
   ▼
PrefixPreparation ── export_artifact ──► KernelArtifact { payload, digest }
   │                                          │
   │                                          ├─ to_json  ──► envelope JSON
   │                                          │
   │                                          └─ from_json ◄── envelope JSON
   │                                                   │
   │                                                   ▼
   │                                          validate + digest + build
   ▼                                                   │
thread(entry, descriptor, inputs) ◄── prepare / prepare_with_limits
   │
   ▼
invoke(args) ──► PrefixTraceReport
~~~

The left and right halves meet at `PrefixPreparation`: a source-backed caller can
walk immediately, and an artifact-backed caller reconstructs the same
representation without Oxc. Complete dispatch remains source-backed; an
artifact-backed trace input snapshot supports prefix mode only.
