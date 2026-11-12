# Actual VM properties (tracer input bundle)

`shape kernel show` prints one concise, **flat sample data configuration** for a
deobfuscated VM copy. It is a bounded Oxc AST read; it never executes
JavaScript, calls `eval`, or runs WebAssembly.

The bundle is not a closure-complete copy of the JavaScript runtime. Operation
handler sources stay verbatim (including whatever shuffled aliases they still
contain), while alias definitions, decoder bodies, helper/function sources,
string caches, native-function tables, runtime helpers, and external-global
lists are intentionally absent: deobfuscation replaces those aliases later.
Known runtime structure stays in the Rust proof/tracing APIs rather than in this
data projection.

This page exists to make the **static data handoff** inspectable. It answers
“what entry values, tables, literal payloads, and VM field roles did the
extractor find in this copy?” It does not answer “can this kernel be traced?”
or “which WASM branch reaches this operation?” Those require the separate
runtime, dispatch, template, and operand proofs described in
[VM flow](../reference/vm-flow.md). Keeping the bundle small makes its unknowns visible:
an omitted coordinate means the reader did not prove it, while a present
literal can be inspected without running the source.

Build the release executable once and put it on `PATH` (or install it) as
described in [Build once](../../README.md#build-once). From the repository root,
run:

```sh
./target/release/shape kernel show tests/fixtures/actual-properties.js \
  --input-kind deobfuscated --pretty
```

`shape` is the sole root Rust executable. `kernel show` accepts any source
filename; `--input-kind raw|deobfuscated` is explicit and never inferred from
the suffix. `--format json|markdown` also accepts `md`, and `--op KEY` is a
repeatable local table-key selection. Flags may appear on either side of the
source. `--pretty` is accepted only for JSON and indents the same parsed values;
Markdown rejects it. Compact JSON remains the default, diagnostics stay on
stderr, and the command does not write files or execute selected handlers.

A complete JSON output for the checked-in fixture is available at
[`tests/fixtures/runtime-config/actual-properties.json`](../../tests/fixtures/runtime-config/actual-properties.json).
That file is the actual output contract example, not the broader illustrative
shape below. It contains four operation keys, 16 strings, two thread
descriptor rows, numeric bytecode, and no WASM module. The optional WASM,
base64, and worker entries in the schema illustration are exercised by
separate focused tests.

For a small, inspectable selection, run the same fixture with `--op 2`:

```sh
./target/release/shape kernel show tests/fixtures/actual-properties.js \
  --input-kind deobfuscated --format json --op 2
```

The operation field of that response is exactly (shown as an excerpt, not a
complete JSON document):

```text
"vmOperations": {
  "2": "function (context) { context.stack.push(context.memory.readSlot(0)); }"
}
```

The root still contains its entry, tables, context/memory/stack roles, and
bytecode; `--op` filters only the proven operation map. In this fixture the
available keys are `11`, `2`, `4`, and `7`; selecting `0` is a usage error
that lists those keys. That makes a missing key distinguishable from a
handler whose source happens to be empty. The checked-in JSON file shows the
full unfiltered response, including `-0`, `NaN`, and `Infinity` representation.

## Flat document

The root is a plain flat object. Each component is exactly one top-level key,
and the actual table/payload names from the capture are retained instead of being
wrapped in `operations`/`strings`/`threads`/`vm`/`data`/`constants` groups.

```json
{
  "entry":              { "pc": 0, "state": 0, "initialSlots": [1, 3, 2, 0] },
  "<operation table>":  { "0": "<exact handler source>", "1": "…" },
  "<string table>":     ["…"],
  "<descriptor table>": [{ "argumentSlots": [], "capturedSlots": [], "initialSlots": [] }],
  "vmContext":          { "byteOffset": { "argument": 0 }, "stack": { "kind": "stack", "writable": true } },
  "vmMemory":           { "slots": { "initial": [], "writable": true }, "methods": ["initializeSlot", "readSlot", "writeSlot", "cloneMemory"] },
  "vmStack":            { "initial": [], "methods": ["pop", "push", "slice", "splice"] },
  "bytecode":           "<plain base64 string>",
  "<module>":           "<plain base64 string>",
  "<memory>":           { "initial": 3, "maximum": 3 },
  "moduleImports":      { "<module>": ["…"] },
  "moduleExports":      { "<module>": ["…"] },
  "<dispatch table>":   [[1, 2], [3, 4]],
  "<constant>":         "scalar-or-array",
  "<worker bootstrap>": { "entryPc": 0, "state": 0, "descriptorIndex": 0, "initialSlots": [] }
}
```

Empty optional components are omitted. The old `bindings`, `external`,
`decoders`, `functions`, dependency metadata, and group wrappers are not emitted
at all. The checked-in fixture's operation table is named `vmOperations`, but
the renderer uses the extractor's actual name; a fixture may name its table
`operations` or anything else. If two extracted components claim the same
top-level name, the first one wins; the renderer does not invent a renamed
key. A consumer that needs to resolve such a collision must use the typed
facts and source evidence rather than treating the flat projection as a
complete binding inventory.

### Loader contract

These are the **producer's bundle shapes**. A future loader can use them as
machine input, but it must validate the fields needed by its tracing mode;
the current tracer does not load this JSON and execute handler strings. In
particular, mere presence of `entry`, `bytecode`, and an operation table is
not dispatch or execution admission.

Read each component this way:

- **`entry`** is an object with only the startup invocation's known positions:
  the second argument is the boot PC, the third is dispatch state, and the
  fourth is the initial slot vector. A concrete VM boot needs numeric PC/state
  and numeric slot indices; the producer currently retains any statically
  known scalar in the first two positions, so a loader must validate the
  coordinate types before using them. The
  host-detection expression, function name, parameter names, and entry
  backreferences are omitted. Unknown or expression-valued positions are
  omitted rather than replaced with a fabricated boot coordinate.
- **Operation keys** are the actual capture table names. Their values are direct
  objects whose keys are the exact local table keys (strings, not necessarily
  sequential) and whose values are the exact handler source strings from the
  standardized output, not the original raw input.
  Repeatable `--op KEY` drops unselected keys from that direct map; it adds no
  `selected` or `count` field and does not change the root shape. Missing keys
  are rejected against the available-key list.
- **String and descriptor tables** are direct arrays under their actual capture
  names, in source order. The checked-in fixture's `vmStringTable` has 16
  entries and `vmThreadDescriptors` has two rows. A descriptor's
  `argumentSlots`, `capturedSlots`, `initialSlots`, and optional `resultSlot`
  describe thread setup; they are not the outer entry's slot vector.
- **`vmContext`**, **`vmMemory`**, and **`vmStack`** are role objects, not
  constructors or JavaScript sources. Each field appears once. A field record
  merges its initializer role and descriptor flags: `{ "argument": index }` when
  the initializer is directly a constructor parameter, `{ "kind": "stack" }`
  when it is the proven stack-constructor result, a literal initializer under
  `"initial"`, and `"writable"`/`"enumerable"`/`"configurable"` flags in the
  same object. The constructor initializer wins; an unused inherited
  `value: null` is never presented as an instance initial value. When only a
  prototype literal is known, a non-null literal is retained as `"default"`, and
  `value: null` is omitted. An unparsed initializer keeps the field name with
  only the flags that are actually known (for example
  `{ "writable": true }`); no initial value is invented. Explicit `false`, `0`,
  `null`, and `[]` values are preserved when they are actual initializers.
- **Methods** are known function-descriptor names (`methods` array), not copied
  bodies. `vmStack.initial` is the stack constructor's returned literal array
  when the extractor proves it; otherwise the key is omitted.
- **Payloads** are plain base64 strings. `bytecode` is one exact encoded
  literal when only one distinct candidate is found, or an array when
  multiple distinct encoded candidates remain. That array is an ambiguity
  for a loader to resolve, not an instruction to concatenate the strings.
  Each `WebAssembly.Module` binding is resolved
  statically to its exact base64 literal under its own actual key (for example
  `vmHandlerTableModule`); each literal occurs once. `WebAssembly.Memory`
  becomes the literal `{ "initial", "maximum" }` values.
- **`moduleImports`/`moduleExports`** are parsed WASM metadata keyed by the
  module's actual binding name. They appear only when parsing succeeds and
  that side has names to report; absence is not proof that a module has no
  behavior or that a dispatch join is known.
- **Numeric bytecode/dispatch constants** and other scalar/array sample
  constants are flat under their actual capture names (for example `vmBytecode`,
  `vmDispatchTable`, `vmNumberConstants`). Native-function arrays, string
  caches, alias/helper tables, `S`-style function dispatch objects, and
  redundant `{ "value", "writable" }` descriptor constants are filtered out.
- **Worker startup** descriptors are flat under their actual binding name and
  carry `entryPc`, `state`, `descriptorIndex`, and `initialSlots`. This is real
  extracted data, not the entry boot state.

Values keep the tagged representation used by the extractor, so `undefined`,
`NaN`, non-finite numbers, negative zero, and array holes stay distinguishable
from strings: `{"$undefined":true}`, `{"$number":"NaN"}`,
`{"$number":"Infinity"}`, `-0`, `{"$hole":true}`. Literal objects whose keys
merely look like metadata remain exact objects. No `$expression` or `$function`
record, payload wrapper (`callee`, `encoded`, `proven`, `arguments`,
`dependencies`, `source`), decoder body, expanded `decoded` byte array,
duplicate binding copy, module-instance expression, or `S` function dump is
emitted.

### Read the checked-in bundle as data

The [fixture output](../../tests/fixtures/runtime-config/actual-properties.json) begins with
`"entry":{"initialSlots":[9,8,7],"pc":17,"state":3}`. Those are known
arguments of the outer startup call. They do not say that the bytecode at PC
17 is in bounds or which operation the dispatch table will choose. The
`vmContext` fields record how a *new* context receives its arguments:
`byteOffset.argument = 0`, `dispatchState.argument = 1`,
`memory.argument = 2`, and `receiverValue.argument = 3`. This is why the
bundle keeps both the startup values and the constructor roles: they answer
different questions.

The fixture's `vmOperations` map has keys `11`, `2`, `4`, and `7`. Key `7`
contains a `function (context, operand) { ... }` source string; its parameter
is visible, but the JSON does not say how an instruction decodes or passes
that operand. `vmBytecode` is `[7,2,11,4]`, a numeric sample table under its
source name. It is distinct from the optional `bytecode` **base64 literal**
extracted from a decoder call in other kernels. The bundle does not equate a
numeric table entry with an opcode or a local handler key without a dispatch
proof.

The first `vmThreadDescriptors` row is
`{"argumentSlots":[4],"capturedSlots":[7],"initialSlots":[0,2],"resultSlot":1}`.
These arrays describe a thread invocation, whereas the outer entry's
`[9,8,7]` initializes the startup context. They must not be combined into
one undifferentiated “initial slots” list. `vmNumberConstants` contains `3`,
`-0`, tagged `NaN`, and tagged `Infinity`; converting the tags to strings or
ordinary JSON `null` would alter the values before any trace begins.

### What a loader still has to establish

The JSON supplies values, not their executable joins. A loader for a
particular tracing mode needs to establish all of these from the current
source/report and its own bounds before running:

1. Choose one concrete startup PC, state, slot vector, and byte image. Reject
   absent, nonnumeric, or ambiguous values required by that mode; do not
   silently use zero or the first base64 candidate.
2. Resolve the operation table's **source identity** and local keys against
   the dispatch selector. A handler key in `vmOperations` is not by itself a
   state/opcode mapping, and `--op` filtering is for display, not a safe way to
   prune a runtime table.
3. Parse or reuse proved handler templates and operands from the same
   standardized source. The bundle omits helpers and aliases, so evaluating
   its handler strings alone would be an incomplete runtime reconstruction.
4. Preserve the order of memory/stack effects and the distinct tagged values
   while applying the tracer's completion and limit rules. Role records such
   as `vmMemory.slots.initial = []` describe construction, not a snapshot of
   every live slot at an arbitrary execution point.

The current complete-dispatch and prefix preparation paths perform their own
source, runtime, template, operand, and selector checks. This list explains
why the flat bundle cannot replace those proofs; it is not a claim that an
unimplemented JSON loader already enforces them.

### Data versus static tracer

This document supplies per-capture **data**. It does not implement the tracer and
does not claim that JavaScript/WASM execution, decoding, dispatch geometry, or
operand semantics are complete. The current symbolic executor consumes proved
handler templates plus caller-supplied decoded operands; the bounded dispatch
drivers separately prove the JS dispatch join, decode operands, and model the
supported stack/memory effects. No current API loads this JSON and interprets
its raw handler strings as a complete VM. Aliases inside handler bodies are
intentionally not normalized here.

The public typed API for an already deobfuscated source is
`shape::extract_actual_properties` (or
`extract_actual_properties_with_limits`), and the renderer is
`shape::tracer_json` / `ActualProperties::tracer_json`. The legacy
typed compact API (`ActualProperties::json` / `compact_json` and the extractor
fields it uses) remains available for consumers that still need the older view.
Use `deobfuscate_with_report(...).output` first for raw input when a report is
needed; for source-only use, `deobfuscate(...)` returns the output string. The command's default
`--input-kind raw` mode performs that hand-off.

## Static layout report

Use the flat `kernel show` bundle when you want the source-shaped data itself:
entry literals, tables, field roles, payloads, and handler snippets. Use the
**verbose** static layout report when you need evidence and relationships:
component spans, source hashes, WASM layout metadata, pending facts, or
optional operation diagnostics. Use [ops inspection](../operations/ops-inspection.md) for
one handler's separate classification, recipe, admission, and operand-lift
statuses. These surfaces share source material but have different contracts;
none is a complete execution trace.

The locator report is built by `static_kernel_layout` (or
`static_kernel_layout_with_options`) and its typed fields (`components`,
relationships, spans, evidence, hashes, and operation facts) remain available.
`KernelLayoutOptions::include_operation_diagnostics` is `false` by default.
When enabled, the same catalogue pass adds an optional
`operation_diagnostics.records` section; it does not change the compact
inventory. Classification and semantic family are bounded recognizer metadata,
not HandlerIR completeness or execution admission, and confidence is derived by
the catalogue from classification rather than copied into each summary.

The verbose serializers are `layout_verbose_json` and
`layout_verbose_markdown`, using the unversioned `shape-kernel-layout`
identifier. The `schema_version` and `analysis_version` field names remain
in the typed/JSON report for callers, but neither value claims a numbered
wire revision. The compact `operations` object contains table identity,
handler/classification counts,
sparse-entry count, and an explicit unknown dispatch-link statement. No
legacy-action crosswalk is part of the model. The default
`layout_json`/`layout_markdown` still render the `ActualProperties` compact view;
the `shape kernel show` command emits the flat tracer-input bundle above.
Neither default path emits spans, evidence, lineage, component records, or
per-handler diagnostics.

WASM dispatch geometry and branch-specific operand semantics remain unresolved.
The properties output retains available module/memory data and exact encoded
decoder literals, and marks non-static pieces as omitted or literal data; it
does not guess a dispatch mapping or operand schema.
