# From capture to executed handler

This page follows a raw capture through extraction, handler inspection, operand
decoding, and a bounded trace. It then shows where the JavaScript and WASM VM
paths differ before they reach the shared prefix walker. The linked module
guides give the exact fields, limits, and serializers for each step.

## The short version

~~~text
raw capture source
      │
      ▼
transforms::deob::pipeline ──► PipelineReport
                            │
                            ├──► kernel facts (layout, proven images, tables)
                            ├──► catalogue (inventory)
                            ├──► HandlerIR graph
                            │          │
                            │          ▼
                            │      recipes ──► HandlerTemplateReport
                            │                       │
                            │             admit_recipe per template
                            │                       │
                            │            ┌──────────┴──────────┐
                            │            ▼                     ▼
                            │     concrete executor    symbolic tracer
                            │     (direct, admitted)   (direct, admitted)
                            │
                            └──► operand descriptors (byte-reading rules)
                            │
                            ▼
dispatch preparation (source + report + entry + completion + limits)
      │  re-proves templates, descriptors, table, and image
      ▼
bounded dispatch execution over one TraceSession
      │  (the symbolic tracer runs each decoded handler inside the loop)
      ▼
DispatchTraceReport
~~~

Nothing in this diagram is a single monolithic command. Each arrow is an
opt-in hand-off; the receiving stage revalidates its input, and dispatch
grounds its proofs in the report rather than trusting a
name or a bare trace.

For a first concrete inspection, use the tracked seven-handler fixture:

```sh
./target/debug/shape ops list tests/fixtures/ops-inspection/handlers.raw.js \
  --format markdown
./target/debug/shape ops show tests/fixtures/ops-inspection/handlers.raw.js \
  --op 0 --format markdown
```

The first command inventories all seven local table keys. The second shows
key 0's ordered micro-ops, extracted recipe, operand proof, and independent
admission status. It does not execute that handler. To move from inspection
to an execution claim, use the bounded dispatch inputs described in
[JSON trace output](../tracing/trace-json.md); an `ops show` classification
alone is insufficient. The fixture deliberately contains both lifted and
partial cases, so it also demonstrates why an inventory row and an admitted
recipe are different things.

## Static extraction

The pipeline front end does not run the capture. It parses the source with Oxc,
applies the recognized extraction and naming passes, and returns both the
standardized JavaScript and the evidence it collected:

~~~text
raw bytes
   │
   ▼
deobfuscate_with_report(source)
   │
   ├─ Oxc parse + semantic pass
   ├─ recognized transforms (strings, aliases, layout/naming, WASM bindings)
   └─ proof + presentation
        │
        ▼
   PipelineReport {
     output,                 standardized JavaScript
     input_hash,             FNV-1a identity of the raw input bytes
     stages, diagnostics,    what ran and what did not hold
     binding_mappings,       renamed binding spans
     property_mappings,      renamed layout/property spans
     binding_residue,        owned names the pass could not rename
     extraction,             static facts (layout, runtime config, tables, …)
     proven_operation_tables table identity: ordinal + shape counts
   }
~~~

Two properties matter for the report-backed stages below:

- `output` is the standardized source that later parsers consume. It is not a
  runtime record and does not imply that the capture was executed here.
- Facts are retained with their evidence and status. A fact that extraction did
  not prove stays missing or diagnostic; it is never replaced by a name-shaped
  guess.

`deobfuscate(source)` is the source-only sibling of
`deobfuscate_with_report(source)`. It returns only the standardized JavaScript
and is appropriate when no downstream analysis needs the facts.

A separate kernel/runtime-config layer reads the same report to retain static
layout, proven JS/WASM image literals, operation tables, dispatch tables, string
tables, descriptors, and related facts. `static_kernel_layout(source)` and
`kernel::config::extract(source)` are source-level entry points that run the
pipeline internally when the caller does not already have a report. These facts
are evidence, not a boot log.

Extraction is a static claim. It does not observe a browser, Node, a WASM
runtime, or a real capture boot, and it does not guarantee that every capture is fully
runnable. Coverage varies by capture, so partial results, `Unsupported`
findings, and unresolved facts remain visible rather than being rounded up to
success.

## Glossary

| Term | What it means here |
|---|---|
| `PipelineReport` | The shared evidence package produced by one pipeline run: standardized JavaScript plus facts, input identity, binding/property mappings, proven operation-table identities, stage reports, and diagnostics. The report-backed analyses below start from this report rather than re-parsing the raw source; source-level entry points such as `deobfuscate` and `static_kernel_layout` also exist and run the pipeline internally. |
| Kernel facts | Static layout, proven image literals, and table/descriptor facts recovered from source plus the report. They describe what the capture contains; they are not a runtime observation. |
| Catalogue | A bounded, best-effort inventory of the proven operation-handler table, exposed by the shape ops views. It classifies handler functions and their spans; it is discovery metadata, not an execution contract. |
| Handler | One capture-local table function: the code reached by a dispatch selector. Its table position is meaningful inside that capture's proven table only. |
| `HandlerIR` | A reusable recovered micro-op program for one handler: typed value and effect nodes with provenance, plus explicit unknown regions. It is a static recovery surface, broader than the executable recipe subset. Formally `HandlerIR` is an alias of `HandlerGraph`. |
| Recipe | The interpreter-oriented projection of a handler graph: an ordered list of steps over operands, temporaries, stack, memory, and selected primitives. A recipe is what the concrete executor and symbolic tracer consume. |
| `HandlerTemplate` | A recipe packaged with identity and status: handler/table keys, semantic identity, source span, `status`, steps, and unsupported fragments. Extraction assigns the status; consumers call `admit_recipe` before execution, and only an admitted `executable` template may run. |
| Operand descriptor | A static rule for reading bytes for one handler: relative offset, width, endianness, mask, signedness, PC delta, and source spans. It describes how to decode, not a decoded value. |
| Decoded value | One concrete operand value produced by applying descriptors to a byte image at a proven PC. Decoded values are per-occurrence inputs to a recipe. |
| `VMInstruction` | An alias of `DecodedInstruction`, carrying only `handler_index` and `operands` for one instruction occurrence. Dispatch PC and VM state stay in dispatch records. |

The count of handlers, instruction formats, and micro-ops is not one-to-one: one
handler function can implement many micro-ops, and one VM instruction selects a
whole handler whose recipe may contain many steps.

## Catalogue, handlers, and identity

The catalogue answers "what is in this proven operation table?" It records each
handler's table key, source span, classification, and structural fingerprint. It
does not decide that a handler is executable, and it does not decode operands or
dispatch.

A handler key is a capture-local fact. The same numeric key in two different captures
may refer to unrelated code, and a familiar-looking name such as `vmOperations`
is never sufficient identity. Across rotating captures the overall structure may
recur, while the tables, decoder parameters, and images differ. Use the proven
table declaration ordinal plus shape validation, and the catalogue's semantic
identity where present, instead of treating numeric keys as cross-capture names.

## `HandlerIR`: recovered micro-op program

`HandlerIR` (`HandlerGraph`) records one handler as a value/effect graph. Nodes
carry source provenance, and edges distinguish value dependencies from effect
order. Regions the extractor cannot prove are kept as explicit unknowns.

~~~text
Conceptual illustration only — not a recovered capture:

        operand 1 ──┐
                    ├──► add ──► t2 ──► stack[0] write
        stack[0] ───┘                   │
                                        └──► effect edge → next operation

        t0 = read stack[0]        (pseudo recipe step)
        t1 = operand 1            (pseudo recipe step)
        t2 = t0 + t1              (JavaScript `+`, not numeric-only add)
        write stack[0] = t2       (pseudo recipe step)
~~~

The steps above are illustrative, not a recovered capture. In the graph form, the
two reads are value nodes feeding one operator node, and the write consumes the
result while participating in the handler's effect order. A handler can contain
several such clusters, including property reads, calls, and unknowns that the
executor will refuse.

The graph is deliberately broader than the executable subset. In the current
implementation it retains object and array literals, non-VM property reads and
writes, captured bindings, and unknown calls as static evidence. Retention is
not permission to run them: the concrete executor rejects object-model and
VM-control nodes before executing a step, and the symbolic tracer handles only
its documented bounded cases.

## Recipes and handler templates

A recipe projects the graph onto the operations an interpreter can handle:
operand reads, temporary locals, literals, stack/memory access, proven PC/state
updates, and selected primitives. `handler_templates_from_report` builds and
classifies the templates; it does not admit them:

~~~text
HandlerIR graph
      │  handler_templates_from_report:
      │  project the executable subset, assign status,
      │  retain unsupported fragments
      ▼
HandlerTemplate {
  handler_index, table_key,       capture-local dispatch keys
  semantic_identity,              catalogue fingerprint or span hash
  source_span,
  status,                         executable | partial | unsupported
  steps, unsupported
}
      │
      ├─ concrete executor: admit_recipe(template, bounds), then execute
      └─ symbolic tracer:   admit_recipe(template, bounds), then trace
~~~

A template is `executable` only when every expression and effect is proved.
Templates with some supported steps and some unknown behavior are `partial`;
templates with no supported step are `unsupported`. Unknown fragments are never
silently turned into no-ops, and both consumers reject non-executable templates
rather than executing a prefix and calling it complete. Consumers call
`admit_recipe` immediately before execution; they do not construct a new
template after admission.

The concrete executor is a bounded interpreter over admitted recipes. It applies
one handler against an explicit stack/memory state and rejects unsupported
values or effects. The symbolic tracer consumes the same recipes but produces
symbolic values and trace events instead of concrete ones.

### Operand descriptors and decoded values

Operand separation produces descriptors from static decoder code. A descriptor
does not contain a value; it says where and how to read one for a given handler,
for example a two-byte little-endian field after the opcode. Applying that
descriptor to a byte image at a proven PC produces the decoded operands that
become `VMInstruction::operands`.

A hypothetical format illustrates the split: given an invented u16
little-endian operand rule, the bytes `07 00` would decode to `7`. This is a
stand-in for the descriptor/decoder distinction, not a claim about any current
capture, whose widths and endianness are whatever its proven descriptors say.

The tracer does not infer operand widths or decode a raw bytecode format on its
own. Callers supply decoded operands, or they use the dispatch path, which
applies the lifted descriptors at the proven coordinate.

## Dispatch: the per-instruction loop

Bounded dispatch connects the proven byte image, the `state -> opcode -> cell`
table, the handler templates, and the operand descriptors. Preparation and
execution are separate steps:

~~~text
prepare_dispatch_from_report(source, report, start, completion, limits)
   │
   ├─ re-derive kernel/runtime-config facts from source and report
   ├─ exactly one proven extracted dispatch table
   ├─ image: one proven extracted bytecode fact, or the proven
   │  direct-bytecode fallback when no extracted fact exists
   ├─ table identity and bounds checks
   ├─ selector → template and selector → operand-descriptor joins
   └─ returns DispatchPreparation (validated, immutable; no handlers run)

prepared.execute(context, trace_limits)
   │
   └─ one persistent TraceSession runs a bounded loop:

        for _ in 0..limits.max_dispatch_steps {
          opcode = image[pc]              // typed fault if image/table lookup fails
          [next_state, selector] = table[state][opcode]

          if selector == completion_selector {
            commit the typed completion value
            stop the dispatch loop
            // CompletedSelector only when the commit succeeds; otherwise a
            // typed stop leaves the attempt uncommitted
          }

          lifted   = operand_handler[selector]   // checked after completion
          operand_entry_pc = proven entry coordinate for this opcode
          operands = decode(lifted, image, operand_entry_pc)
          next_pc  = operand_entry_pc + lifted.pc_delta

          run the handler symbolically in the shared session
          // a throw, unsupported recipe, or budget is a typed stop

          transfer = handler control transfer   // jump/call/return, if any
          pc    = transfer.byte_offset    ?? next_pc
          state = transfer.dispatch_state ?? next_state
        }
~~~

`prepare_dispatch_from_report` validates and prepares; it does not execute.
Preparation re-derives the runtime config, but it does not require an extracted
bytecode fact: when the report has no proven extracted image, preparation uses
the proven direct-bytecode fallback from the standardized source.
`DispatchPreparation::execute` (or a bound `DispatchInvocation`) runs the
bounded dispatch through one persistent symbolic `TraceSession`, so stack,
memory, and arena state are shared across committed handler occurrences. An
unsupported or budget-stopped handler attempt does not commit a coordinate, and
earlier committed records are never erased. A thrown handler is different: its
effects and terminal record are retained and reported with the failed
coordinates.

The caller supplies the entry `DispatchStart { pc, state }` and the typed
completion contract. No constructor argument, outer kernel call, or familiar
name is guessed as the entry. The contract names the sentinel selector and its
value policy (`Stack { index }` or `Undefined`); preparation validates both. The
loop checks that selector before it resolves a normal handler, decodes operands,
or runs a recipe. An ordinary handler `return` returns to the loop.

A run can also stop before the completion selector at a table, image, operand,
or step fault; at a repeated `(pc, state, call stack)`; on a thrown handler; on
an unsupported recipe or operand; or when a runtime limit is exhausted. These
are typed `DispatchStopReason` values and the trace records the matching
diagnostic. Only a `CompletedSelector` stop whose completion value was committed
is a successful completion
(`DispatchTraceReport::is_complete_completion`); every other stop is partial or
failed.

`VMInstruction` is the runtime-facing name for one instruction occurrence. It is
an alias of `DecodedInstruction` and holds exactly `handler_index` and
`operands`. The dispatch PC, VM state, opcode, and occurrence location live in
the surrounding dispatch record, not in the instruction. Keeping them separate
prevents a decoded operand bag from being mistaken for a location in the loop.

## Where JavaScript and WebAssembly VM paths diverge

Both variants eventually need the same semantic question answered: which
handler ran with which operands, against which VM stack/memory state? They do
not obtain the dispatch coordinate or operands the same way. The JavaScript
VM's loop and inline decoder are in the standardized JavaScript. The WASM
VM's loop and byte loads are in the interpreter module; its JavaScript imports
supply handlers and host helpers. A successful static inventory of imports is
therefore only an input to the WASM proof, not a completed execution.

For the tracked paired `readSlot` case, the JavaScript VM fetches an opcode and
leaves `context.P` pointing at the next byte. Its handler reads
`r[P] | r[P + 1] << 8`, advances `P` by 2, then calls
`context.memory.readSlot(slot)`. The WASM interpreter retains the opcode PC,
loads the same u16 little-endian operand at `local0 + 1`, advances `local0` by
the instruction width, and passes the decoded value to a handler parameter.
For an opcode at 1000 and bytes `34 12`, both produce slot 4660 and next PC
1003, though one starts its operand read at stored `P = 1001` and the other
at stored `local0 = 1000`. The other four paired cases, including the crucial
`u8 + u16le` versus `u8 + u8 + u8` distinction, are documented in
[JS/WASM operand cases](js-wasm-operand-cases.md). Equal PC deltas alone do
not prove equal field boundaries.

On the JavaScript path, `prepare_prefix_from_report` proves the source run
loop, operation table, image access, patch handler, and recipe/operand joins
before the prefix walker executes. The complete-dispatch path additionally
requires a caller-supplied completion selector and returns a
`DispatchTraceReport` whose `CompletedSelector` stop is the success criterion.
A prefix walk can instead stop at a supported boundary and still retain its
partial trace. Neither path guesses an entry or constructor arguments.

On the WASM path, the static [boundary inventory](wasm-boundary.md) first
identifies modules, imports, exports, tables, calls, and JS instance bindings.
`kernel::wasm_lift` reads the interpreter bytes to name each selector's
behavior. `dispatch::prefix::wasm` converts those proven selectors to recipes
for the same prefix walker: operation calls, jumps, calls/returns, exception
bookkeeping, image writes, and typed stops. Preparation must additionally
prove the handler-module import-to-`vmOperations` mapping key for key and the
assumed helper imports. The interpreter is not executed by a WASM engine during
this trace; the source-proven behavior is lifted to the shared walker. The
WASM machine also has bounded try, error-record, and live-call banks that the
JavaScript VM path does not impose. See the
[prefix module guide](../../src/dispatch/prefix/README.md) for the module map and
[WASM prefix preparation](../tracing/wasm-prefix.md#wasm-dispatched-kernels-prefixwasmrs)
for the exact handoff and limits.

The two exception paths also differ at the host boundary. The compared
WebAssembly interpreter captures have no WASM exception-handling instructions. A host import
throw leaves the WASM activation as a JavaScript exception; a separate mutable
global signals the interpreter's unwind request, while the thrown value lives
on the JavaScript error stack. [WASM exception boundary](wasm-exception-boundary.md)
separates that flag from the value and lists the source-backed function/byte
offsets. Treating the flag as a payload, or assuming a JavaScript handler
exception becomes a WASM `throw` instruction, would give the wrong state.

## Open joins and how to recognize them

The [`wasm-boundary.md`](wasm-boundary.md) adapter inventory can prove that
an import has namespace `q` and name `"7"` without proving that its copied
`vmHostBindings` closure came from `vmOperations[7]`. Its mapping count remains
zero in that case. The structurally admitted WASM prefix path demands the
stronger provenance before it walks. This explains why a module can be
inventoried but fail lift/preparation, and why no import spelling alone should
be used as a handler key.

The historical paired operand cases establish five handler-family
correspondences, not a complete branch-selector-to-handler table. Likewise,
`HandlerIR` can preserve an effect that an executable recipe cannot represent,
and a template can be executable while a particular dispatch entry or operand
width is unproved. When a row is unknown, record the missing join (table
binding, selector, operand origin, helper identity, or completion contract)
with its source coordinates. “TBD” without that missing relation is not useful
evidence. The tracked WebAssembly prefix regression demonstrates one
successful admitted walk; it is not a blanket guarantee for another capture.

## Concrete vs symbolic consumers

Both consumers read admitted recipes; they differ in what a value is:

~~~text
              HandlerTemplate (status == executable)
                              │
                    admit_recipe(template, bounds)
                              │
              ┌───────────────┴────────────────┐
              ▼                                ▼
     concrete executor                 symbolic tracer
     values are concrete               values are symbolic
     stack/memory hold data            stack/memory hold value nodes
     own execution limits              own trace limits
     result: concrete state            result: TraceReport + events
~~~

These are alternatives, not a required sequence. Symbolic tracing does not need
a concrete run first, and the concrete executor does not produce the symbolic
tracer's node/event report. Both call the shared `admit_recipe` structural gate,
but they do not share one runtime bound: each consumer supplies its own limits
and execution policy. The concrete executor additionally rejects object-model
and VM-control nodes, while the symbolic tracer supports its documented subset.

Bounded dispatch is a third caller. It uses the symbolic tracer internally for
each decoded handler, so the tracer is part of the dispatch loop rather than a
separate pipeline reached from descriptors alone.

## What this path does not claim

- **No runtime observation.** Extraction and analysis are static. They do not
  prove that a capture booted, that a handler ran in a real VM, or that an observed
  capture matches the static image.
- **No full-capture guarantee.** The supported recipe subset is intentionally
  narrow. Many handlers are `partial` or `unsupported`, and the raw JavaScript
  VM capture remains outside the complete-dispatch contract.
- **No automatic promotion.** A catalogue entry, a handler key, or a recovered
  graph fragment does not authorize execution. A template must have
  `executable` status and pass `admit_recipe`, and dispatch must be prepared
  from a source-validated proof.
- **No cross-capture names.** Numeric keys are local to one proven table. Rotating
  captures reuse structure, not identities.
- **No silent unknowns.** Missing, ambiguous, or unsupported facts stay visible
  in statuses, diagnostics, and trace stops.

The traced JavaScript VM program exposes a serializer that reads
`positionalSignals`, `signals`, and `keyedSignals`, writes bytes, and returns an
encoded string. That observation is not a capture-specific encoder or decoder:
the tracked fixtures contain no fixed-input signal-stage and payload-stage
reference byte pair. A codec claim requires those bytes and a proven path from
the positional input through its length/append operations, encryption, and
final encoding. Do not substitute bytes from the old local generator.

## Further reading

Module and contract guides:

- [Deobfuscation workflow](../deob/deobfuscation.md) — pass order and supported
  inputs.
- [Kernel layout and tracer input](../deob/kernel-layout.md) — static layout, images,
  tables, and the tracer bundle.
- [Operation catalogue](../operations/operations.md) — catalogue scope and JSON contract.
- [Handler templates](../operations/handler-templates.md) — recipe subset and admission
  status.
- [Bounded handler executor](../operations/handler-executor.md) — concrete one-handler
  execution.
- [JavaScript operand separation](../operations/js-operand-separation.md) — descriptors and
  inline operand reads.
- [Symbolic tracer](../tracing/symbolic-tracer.md) — `VMInstruction`, `TraceContext`, and
  bounded dispatch/thread APIs.

Code entry points and file maps:

- [`src/transforms/deob/pipeline.rs`](../../src/transforms/deob/pipeline.rs) —
  `PipelineReport` and `deobfuscate_with_report`.
- [Handler IR](../operations/handler-ir.md) — graph types, construction,
  validation, provenance, and unknown regions.
- [Handler templates](../../src/operations/templates/README.md) — extraction,
  recipe construction, and `admit_recipe`.
- [Operand separation](../../src/operations/operands/README.md) — binding proof,
  lifting, descriptors, and bounded decoding.
- [`src/operations/executor.rs`](../../src/operations/executor.rs) — concrete
  bounded recipe execution.
- [`src/kernel/config.rs`](../../src/kernel/config.rs) — runtime-config facts and
  artifact identities.
- [`src/kernel/layout.rs`](../../src/kernel/layout.rs) — static kernel layout.
- [`src/dispatch/prepare.rs`](../../src/dispatch/prepare.rs) and
  [`src/dispatch/execute.rs`](../../src/dispatch/execute.rs) — preparation and
  the bounded dispatch loop.
- [Symbolic tracing](../../src/tracing/symbolic/README.md) — the value/event
  model, `TraceSession`, execution, and emission; [session
  internals](../../src/tracing/symbolic/session/README.md) map rollback, branches,
  loops, and subroutines.
