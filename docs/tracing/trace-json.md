# JSON trace output

Add `--json` to `shape trace thread`, `dispatch`, `program`, `corpus`, or `cleanup-thread`.
Keep the normal input files and flags. Stdout contains exactly one JSON document; summaries go to stderr.
No separate command, request router, or versioned request envelope is needed.

```sh
shape trace thread kernel.raw.js --pc 115869 --state 7 --descriptor 31 --json > thread.json
shape trace program kernel.raw.js --thread-cleanup --out program.js --json > program-result.json
shape trace corpus kernel.raw.js --fast --json > corpus.json
shape trace cleanup-thread thread.js --json > cleanup.json
```

## Result format

All modes share this envelope:

```json
{
  "version": 1,
  "command": "thread",
  "status": "complete",
  "result": {}
}
```

`status` is `complete`, `partial`, or `failure`. `result` depends on the command:

| Command | Result |
| --- | --- |
| `thread` | Entry, source hash, stop, completion, steps, handler count, diagnostics, branch decisions, self-check and scope findings, JavaScript |
| `dispatch` | Entry, source hash, stop, completion, tagged completion value, steps, handler count, diagnostics, JavaScript |
| `program` | Traced count, stopped functions and reasons, required kernel globals, linked JavaScript |
| `corpus` | Capture fingerprints, regressions, baseline changes, and whether a baseline was updated |
| `cleanup-thread` | Cleanup counts and cleaned JavaScript |

Without `--out`, `result.javascript.code` contains the JavaScript string and `status` inside that object is `emitted`.
With `--out`, JavaScript is saved there; the JSON gives `status: "written"`, `path`, `bytes`, and `code: null`.
A stopped single trace has `javascript.status: "not_emitted"`. Thread `--partial` permits an explicitly marked
`// INCOMPLETE:` diagnostic prefix; `javascript.incomplete` is true and the command still exits 3.
A partial program contains throwing stubs for stopped functions.

Errors return `status: "failure"` with `result.error` and `result.exit_code`. A stdout transport failure can prevent
JSON delivery. Help remains ordinary help text. Existing exit codes are preserved: thread/program stops exit 3;
thread scope/self-check failures exit 4/5; runtime errors exit 1 and usage errors exit 2. Corpus exits 1 for regressions,
but an incomplete corpus without regressions can still exit 0; inspect its status and fingerprints for coverage.

## Explicit input snapshots

`thread` accepts `--inputs FILE` independently of `--json`. This supplies caller-declared arguments,
captured memory, incoming stack, an arguments object, self value, and receiver. It does not prove those inputs from source
or share live memory with a parent invocation. Snapshot JSON is bounded to 16 MiB. Unknown fields are rejected.

For `thread`, select the descriptor using `--descriptor N` or `--initialslots SLOTS` as usual. Omit `descriptor` from its
snapshot; this avoids two competing descriptor definitions. Supplying `--inputs` replaces the default unknown input
snapshot, so provide all captures and special values required by that descriptor. Omitted fields are empty.

Supported fields are `captured_memory` (an object keyed by slot number), `incoming_stack` (an array), `arguments`,
`arguments_object`, `self_value`, and `receiver`. Dispatch snapshots may also contain `descriptor`, with `initial_slots`,
`captured_slots`, `argument_slots`, `arguments_slot`, and `result_slot`.

Values use explicit tags: `undefined`, `null`, `boolean`, `number`, `string`, `object` (opaque `identity`), or `external`
(opaque symbolic `name`). In `incoming_stack`, JSON `null` is a hole; `{ "kind": "undefined" }` is an explicit value.
Numbers use `value` for finite numbers, or `encoding` for `negative-zero`, `nan`, `positive-infinity`, or
`negative-infinity`. `computed` and `unknown` are output-only tags and are rejected as caller inputs.

Rust integrations use the existing public dispatch/prefix preparation, `ThreadInputs`, invocation, and emission APIs;
see the [SDK guide](../../sdk/rust/README.md).
