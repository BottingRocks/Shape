# Static operation catalogue contract

Status: opt-in static catalogue and inspection view. The catalogue parses
JavaScript with Oxc and classifies the already-proven operation-handler table.
It does not execute JS or WASM, decode the program image, build a complete
tracer/VM, or rename functions. Handler templates, operand separation, and bounded
dispatch are separate consumers of the report and are
implemented elsewhere; catalogue generation remains outside the default
deobfuscation stage.

The catalogue answers a narrower question than "what does this program do".
For one proven operation table it records which numeric entries are handlers,
which bounded syntactic facts can be recognized in each body, and which facts
remain unproven. That is an inventory plus evidence, not an execution plan. A
handler can be visible, classified, and fingerprinted while its recipe is
partial and its operand bytes are unknown; the inspection view keeps those
answers separate.

## Purpose and scope

The catalogue consumes one post-presentation `PipelineReport`: standardized
`report.output` and the `proven_operation_tables` hand-off (ordinal, names,
element/function counts). It does not inspect raw input again.

It produces one `OperationCatalogue`: table identity, inventory counts, one
`OperationRecord` per handler, and fingerprint groups, not a recipe, operand
descriptor, dispatch decision, or value. Those belong to
[handler templates](handler-templates.md),
[JavaScript operand separation](js-operand-separation.md), and
[bounded dispatch](../tracing/javascript-vm-prefix.md).

The distinction cuts both ways: `atomic` is not executable, an empty unknown
list is not admission, and an unexecutable handler still has evidence.

## Choose the operation view

Use the smallest view that answers the question; these surfaces do not share a
status vocabulary.

| Need | Surface | Joins analyses? | Contract |
| --- | --- | --- | --- |
| Which keys exist, and how are they broadly classified? | `shape ops list` | No | Unversioned markdown/JSON |
| What is one handler, with recipe and operands? | `shape ops show --op KEY` | Yes, by proven `table_key` | Unversioned markdown/JSON |
| What operation class does each handler have across captures? | `shape ops classify` | No | Separate classifier |
| What is the exact serialized catalogue? | `catalogue_from_report` / `catalogue_json` | No | `shape-kernel-operation-catalogue/v1` |
| Where does an operand byte come from? | `ops show` plus operand separation | Yes | Operand contract |

The catalogue's `atomic`/`composite`/`unknown` classification is **not** the
`ops classify` vocabulary (`javascript`, `vm`, `fused`, `unrecognized`). The
catalogue classifies AST recognizer results; `ops classify` evaluates handler
recipes against an abstract operand stack. They can disagree, and neither
grants execution.

## Build and run

Build the release executable once as described in
[Build once](../../README.md#build-once). To inspect one handler in the tracked
capture:

```sh
shape ops show tests/fixtures/kernels/javascript/kernel.raw.js \
  --op 0 --format markdown
```

The command forms are:

```text
shape ops list <source> [--format json|markdown] [--pretty]
shape ops show <source> --op KEY [--format json|markdown] [--pretty] [--annotation TEXT]
shape ops classify <source>... [--format json|markdown] [--pretty] [--ops]
```

| Flag | Applies to | Behavior |
| --- | --- | --- |
| `--format json\|markdown` | All three | `md` is accepted as a markdown alias |
| `--pretty` | List, show, classify | JSON only; markdown plus `--pretty` is a usage error |
| `--op KEY` | Show | Exactly one non-negative `u32` table key |
| `--annotation TEXT` | Show | Display-only note; never changes a status |
| `--ops` | Classify | Adds the per-handler list to the markdown summary |

List/show default to JSON; classify to markdown. Repeated or unknown options, a
missing input, or an extra positional argument are usage errors.

| Code | Meaning |
| --- | --- |
| 0 | Success, including a closed downstream pipe |
| 1 | Operational failure: unreadable input, deobfuscation failure, no proven table, serialization or write failure |
| 2 | Usage error: bad/duplicate/missing option, unsupported format, unknown operation key |

An absent but numeric key prints the available keys (usage 2); no proven table
is operational failure 1, not an empty list. Data goes to stdout, diagnostics
to stderr; there is no output-file option and no execution.

Input is read and hashed as raw bytes, then decoded as UTF-8; no suffix is
required. `ops classify` takes multiple file names; list/show take one.

## Where the catalogue sits in the pipeline

```text
raw input bytes
   ├─ SHA-256 ─────────────────► capture identity (ops JSON)
   └─ deobfuscate_with_report
          └─ PipelineReport
                ├─ output: standardized JavaScript
                ├─ input_hash: FNV-1a-64
                └─ proven_operation_tables[0]
                          └─ catalogue_from_report → OperationCatalogue
                                   └─ ops list / ops show / catalogue_json
```

`catalogue_from_report` takes `proven_operation_tables[0]`. A report with no
proven table is a hard error (`NoProvenTable`); the catalogue never synthesizes
a table from a name, a function-valued array, or a likely position. Additional
tables in the vector are not catalogued.

`ops list` and `ops show` run the deobfuscation report per invocation, then
catalogue its output; they do not read a stored report. `ops show` also runs
`Analyses::from_report` for template and operand projections. Every span and
snippet is an offset into `report.output`, never into raw input.

## Identity fields named `source_hash`

Several records use the spelling `source_hash` for different bytes and
algorithms. Read the owner, not the spelling:

| Field | Computed over | Algorithm | Owner |
| --- | --- | --- | --- |
| `capture.identity` | Exact raw bytes, before UTF-8 | SHA-256 | `ops` JSON capture object |
| `catalogue.source_hash` | Raw pipeline input (`report.input_hash`) | FNV-1a-64 | `OperationCatalogue` |
| `Analyses.source_hash` | Raw pipeline input (`report.input_hash`) | FNV-1a-64 | `Analyses` (`ops show`) |
| `HandlerIR.source_hash` | Standardized source (`report.output`) | SHA-256 | Top-level handler graph |
| `HandlerIR.raw_input_hash` | Raw pipeline input (`report.input_hash`) | FNV-1a-64 | Top-level handler graph |
| `occurrence.provenance.source_hash` | Standardized source (`report.output`) | SHA-256 | Each occurrence in a handler graph |
| `occurrence.provenance.raw_input_hash` | Raw pipeline input (`report.input_hash`) | FNV-1a-64 | Each occurrence in a handler graph |
| `capture.source_hash` (occurrence view) | Prefix-preparation input | FNV-1a-64 | Occurrence/decode JSON |
| `capture.artifact_source_hash` | Standardized output parsed during preparation | FNV-1a-64 | Occurrence/decode JSON |
| `span_source.identity` | Standardized source (`report.output`) | SHA-256 | `ops show` span object |
| Structural fingerprint | Canonical normalized body | FNV-1a-64 | Handler record |
| Kernel identity | `CATALOGUE_VERSION:source_hash` | Composition | Handler record |

The catalogue and `Analyses` fields join over the raw input.
`HandlerIR.source_hash`, `occurrence.provenance.source_hash`, and
`span_source.identity` identify the standardized source;
`HandlerIR.raw_input_hash` and `occurrence.provenance.raw_input_hash` keep the
raw-input join identity alongside the standardized-source hash. The
occurrence/decode `capture.source_hash` and `capture.artifact_source_hash` are
preparation identities for different bytes, not aliases of the handler-graph
fields. A comparison across the raw-input FNV family and the
standardized-source SHA-256 family is not a comparison of the same bytes. These
answer different questions: capture digest for supplied bytes, FNV for the
analyzed input, span digest for standardized output. None substitutes for
another; an uncompared digest is unverified.

## Table identity and scanning

`ProvenOperationTable` carries `declaration_ordinal`, `original_name`,
`canonical_name`, `initializer_elements`, and `initializer_functions`. Identity
is the ordinal plus exact shape and counts. The ordinal is the document
position of the target `VariableDeclarator` across the program; it survives
renaming and code generation. The scanner counts every declarator and never
searches by spelling.

`table_scan.rs` is shared, but the scan policies are intentionally different.
The catalogue and templates use `TableScanMode::Strict`; operand separation uses
`Lenient` to retain entries it can still examine. Do not merge the policies to
"simplify" them: their output is versioned and historically fixed.

| Table shape | Strict acceptance | Element count | Function count |
| --- | --- | --- | --- |
| Array | Every slot; function and arrow elements are handlers, holes are not | Array length | Function/arrow elements |
| Numeric-key object | Only plain properties whose values are functions; canonical numeric keys; sorted by key | `max_key + 1` | Accepted function properties |

A canonical key is a finite, non-negative integer in range, or a canonical `u32`
string; `01`, `1.0`, negatives, and symbolic keys are not.

The scan produces `shape` and validation counts. `catalogue_source` fails with
`TableIdentityMismatch` when the ordinal is absent or counts differ.

`handler_index` and `table_key` are separate fields; the catalogue assigns both
from the same proven key. Select by `table_key`, never vector position; the list
prints both because sparse tables make the distinction observable.

One historical detail is preserved: catalogue object counts use
`max_key + 1` (unchecked), while templates use saturating addition. Keep both
methods; a table near `u32::MAX` would otherwise change identity.

## What the catalogue proves, and what it does not

| Claim | Established by | Not established |
| --- | --- | --- |
| This is the operation table | Ordinal, shape, element/function counts | That handler meanings are understood |
| This row exists for a key | Strict scan of the proven declarator | That the handler is executable |
| The body is `atomic`/`composite`/`unknown` | Bounded recognizers and retained unknown fragments | Exhaustive semantic coverage |
| Confidence is `high`/`partial`/`unknown` | Derived from classification, with a fixed proof rule | Empirical accuracy or runtime behavior |
| Two bodies are one fingerprint group | Equal fingerprint **and** equal canonical body | Semantic equivalence |
| This key selects this handler in this capture | The proven table and scan | A cross-capture operation name |
| This call is a known helper | Canonical helper spelling or a report-backed name | Dynamic dispatch or a runtime callee identity |
| This fragment is unknown | The recognizer declined it | That the handler is broken |

The catalogue is fail-closed about evidence and fail-open about visibility: it
records what it cannot recognize instead of dropping the handler. `unknown` is
a normal outcome, and an empty unknown list is not by itself sufficient for
execution.

## Versioned JSON contract

The top-level schema version is `shape-kernel-operation-catalogue/v1`.

```json
{
  "schema_version": "shape-kernel-operation-catalogue/v1",
  "catalogue_version": "shape-oxc-deob/operation-catalogue-v1",
  "source_hash": "008dc2eb1ed18058",
  "table": {
    "declaration_ordinal": 64,
    "original_name": "Ou",
    "canonical_name": "vmOperations",
    "shape": "array",
    "element_count": 201,
    "function_count": 201
  },
  "inventory": {
    "exact": 0,
    "partial": 201,
    "unknown": 0,
    "missing_sparse_entries": 0
  },
  "handlers": [
    {
      "kernel_identity": "shape-oxc-deob/operation-catalogue-v1:008dc2eb1ed18058",
      "handler_index": 0,
      "table_key": 0,
      "source_span": {"start": 73405, "end": 73520},
      "source_snippet": "function(context, value) { ... }",
      "structural_fingerprint": "303843e687fde86e",
      "canonical_normalized_body": "function(b0,b1){return b0+b1;}",
      "semantic_family": "composite",
      "classification": "composite",
      "ordered_micro_operations": [],
      "effects": {},
      "confidence": {},
      "unknown_fragments": []
    }
  ],
  "fingerprint_groups": []
}
```

The example abbreviates nested fields. Fingerprints are candidate grouping
keys, not equivalence proofs: a group compares hash and body, and
`frequency_across_seeds` is the within-catalogue group size, independent of
classification.

| Top-level field | Meaning |
| --- | --- |
| `schema_version`, `catalogue_version` | Serializer contract and analysis identity |
| `source_hash` | Input identity the catalogue was derived from |
| `table` | Proven table identity and counts |
| `inventory` | Counts derived from handler classifications |
| `handlers` | Ordered `OperationRecord` rows |
| `fingerprint_groups` | Duplicate-body groups keyed by fingerprint and body |

| Table field | Meaning |
| --- | --- |
| `declaration_ordinal` | Document-order `VariableDeclarator` index |
| `original_name` / `canonical_name` | Pre/post-presentation names |
| `shape` | `array` or `numeric-key-object` |
| `element_count` | Array length, or `max_key + 1` for an object |
| `function_count` | Accepted handler functions |

| Inventory field | Source |
| --- | --- |
| `exact` | `atomic` handlers |
| `partial` | `composite` handlers |
| `unknown` | `unknown` handlers |
| `missing_sparse_entries` | `element_count - function_count`, saturating |

| Record field | Meaning |
| --- | --- |
| `kernel_identity` | `CATALOGUE_VERSION` plus `source_hash` |
| `handler_index`, `table_key` | Row index and capture-local selector key |
| `source_span` / `source_snippet` | UTF-8 span in standardized output and its text |
| `structural_fingerprint`, `canonical_normalized_body` | FNV-1a-64 of the canonical body, and the body |
| `semantic_family`, `classification` | Family label and closed class |
| `ordered_micro_operations` | Recognized actions in source/AST order |
| `effects` | Aggregate stack, memory, control, exception, and call facts |
| `confidence` | Derived level, proof rule, and evidence |
| `unknown_fragments` | Declined spans with reason and body |

`OperationRecord::confidence` is computed from `classification`, not stored
beside it, so a record cannot claim `atomic` with an unrelated proof rule. The
serializer hand-orders the handler object: `confidence` appears between
`effects` and `unknown_fragments`, which is not the Rust struct field order.

The layout is byte-stable: top-level fields are indented two spaces one per
line, nested values are compact, control characters use `\u00XX` (including
DEL/C1 and long backspace/form-feed), and the object ends with a blank line
before `}`. Do not use default `serde_json`; a test pins the bytes.

## Handler records and classification

Classification is a closed rule over emitted actions and retained unknowns:

| Recognized actions | Unknown fragments | Classification | Semantic family |
| ---: | --- | --- | --- |
| 0 | Any | `unknown` | `unknown` |
| 1 | None | `atomic` | That action's kind |
| 1 | Some | `unknown` | That action's kind |
| 2 or more | Any | `composite` | `composite` |

The one-action-plus-unknown row is subtle: the family names the action, but
classification stays `unknown` because part of the body is unproven. Atomic
does imply one action and no unknowns; family alone does not imply a class.

Confidence is derived with fixed rules:

| Classification | Level | Proof rule | Evidence |
| --- | --- | --- | --- |
| `unknown` | `unknown` | `OP-UNKNOWN-001` | No bounded recognizer matched; body retained verbatim |
| `atomic` | `high` | `OP-AST-ATOMIC-001` | One exact AST recognizer matched and no unknown fragments were observed |
| `composite` | `partial` | `OP-AST-COMPOSITE-001` | Ordered exact recognizers retained; unsupported fragments remain explicit |

`ordered_micro_operations` is a sequence, not a set. Each `MicroOperation`
carries `kind`, an optional operator, operand-role labels, and a source span.
Roles are descriptive positions, not decoded values. `effects` aggregates
stack/memory counts and slots, control-flow and exception names, constructor and
thread counts, and `calls_unknown_possible_throw`.

`semantic_family` is a taxonomy label over the same output, not a semantic
handler name, and it does not survive a capture boundary: one label in two
captures can come from different handlers.

## Canonical body and fingerprints

The canonical body makes structural comparison useful across alpha renaming and
table shuffles:

1. Collect binding identifiers and resolved references, sorted by span.
2. Replace each with `b0`, `b1`, ...; free or unresolved references keep their
   spelling.
3. Splice replacements into the exact source, leaving literals and punctuation
   intact, then compact whitespace to one space.

The structural fingerprint is FNV-1a-64 over that body. Alpha-renamed copies
and shuffled table order share fingerprints; a different literal, operator,
property name, or free helper name changes the body and usually the fingerprint.

Fingerprint groups are keyed by the **pair** `(fingerprint, canonical body)`,
so a 64-bit collision cannot merge unlike bodies. Equal groups are not
semantic-equivalence proofs: two same-shaped handlers can read different
captured values; the group says their retained shapes normalized identically.

## The recognizer boundary

The recognizer is a bounded `Visit` walk over the handler body. It emits a
closed set of micro-operation kinds:

| Group | Kinds |
| --- | --- |
| Arithmetic, access | `js_add`, `js_subtract`, `js_xor`, `js_binary`, `property_read`, `property_write` |
| Stack / memory | `stack_read`, `stack_write`, `memory_read_slot`, `memory_write_slot`, `memory_initialize_slot` |
| Calls | `call`, `constructor_call`, `thread_capture`, `shared_string_decode` |
| Control | `return`, `throw`, `branch`, `loop` |

The first bounded rules are:

- `stack_read`/`stack_write` and the memory operations for the standardized
  `vmStack`/`vmMemory` receiver spellings. These are report-canonical, not
  arbitrary source names; recognition is by spelling, not a `SymbolId` proof,
  and no runtime receiver is resolved.
- `js_add`, `js_subtract`, and `js_xor` (a `+` remains JavaScript addition,
  including possible concatenation/coercion); every other binary operator is
  `js_binary` with operator text retained.
- property read/write, with static and computed accesses distinguished by
  operand roles. The catalogue records the access; it does not model a getter's
  possible throw as a call effect.
- proven `vmDecodeString` helper calls only; a string name or call shape alone
  does not prove the shared decoder. The name must be report-backed.
- thread capture only for an explicitly proven
  `vmCaptureThread`/`vmThreadCapture` binding; return, throw,
  `if`/`while`/`for`/`do-while`, calls, and constructors are also actions.

Call handling is the most consequential boundary:

| Callee form | Result | Unknown fragment / throw flag |
| --- | --- | --- |
| `vmStack.pop`/`read`/`push`/`write`; `vmMemory.readSlot`/`writeSlot`/`initializeSlot` | Stack/memory read, write, initialize | No |
| Proven `vmDecodeString`; `vmCaptureThread`/`vmThreadCapture` | `shared_string_decode` / `thread_capture` | No |
| Other static/direct or computed callee | `call` | Yes: unknown helper/target; possible throw |
| `new` expression | `constructor_call` | Yes: possible throw |

For a recognized helper call, only arguments are visited; the receiver does not
become a `property_read`. For an unrecognized call, the whole expression is
walked, so nested actions can be added; one source line can yield several
micro-operations.

Calls remain possible-throw effects only for unknown callees or `new`; proven
helper calls do not set `calls_unknown_possible_throw`. No JavaScript runtime
inference is used. Handlers with multiple actions keep source order and are
composite. `if`, `while`, `for`, and `do-while` are actions rather than
flattened sequences; other statements are walked for contained actions. Nested
function/arrow bodies stay unknown fragments. Unfamiliar handlers are `unknown`
with body and provenance retained; no universal coverage is claimed.

The 201 JS handlers and 234 WASM dispatch branches in the tracked WebAssembly
VM reference are deliberately not a 1:1 mapping; see the
[source anchors](../reference/operation-reference.md#source-anchors-for-the-fixture). Encoded
state/opcode, operand widths, next state, and linkage are a separate hand-off.
Bounded dispatch proves its own `(current_state, opcode) -> (handler_index,
next_state, operand_widths)` join; it does not use catalogue classification as
execution authority. Until a table is proven, the catalogue must not invent an
encoding, width, or mapping.

## Inventory and coverage

Inventory is derived from classification counts, not measured separately:

`exact` counts `atomic`; `partial` counts `composite`; `unknown` counts
`unknown`; `missing_sparse_entries` is `element_count - function_count`.

`inventory` counts handler rows, not decoded bytes, recipe steps, executed
instructions, or semantic coverage. `partial: 201` means 201 rows were
classified composite; it does not mean 201 handlers are "partly executable",
and it does not predict admission.

`ops show` also reports `handler_templates.coverage`
(`executable`/`partial`/`unsupported`) and `operand_separation.coverage`
(`lifted`/`partial`/`unresolved`/`not_applicable`). These count their own
analyses; never sum them with inventory or each other.

## `ops list` and `ops show`

`ops list` is catalogue-only: capture SHA-256/kind/length, analysis version and
legacy FNV hash, table identity and counts, inventory, and handlers in table
order.

The markdown table has key, index, family, class, micro-ops, confidence, and
unknown count. JSON rows carry `key`, `index`, `family`, `classification`,
`micro_ops` (kinds), `confidence`, `unknown_fragments` (count), and
`fingerprint`. The list never joins analyses and is byte-pinned.

The seven-handler fixture is the fastest way to read the projection:

```sh
shape ops list tests/fixtures/ops-inspection/handlers.raw.js --format markdown
```

Key 0 is `composite` with partial confidence, 20 ordered micro-operations, and
three unknown fragments. That is static inventory, not execution: `ops show --op
0` shows a partial extracted recipe while operand lifting certifies one two-byte
little-endian operand and the lifted recipe is admitted. The four statuses stay
independent.

`ops show` also supplies `span_source`: `kind: "deobfuscated"`, the SHA-256 of
the exact `report.output` bytes, and its length. Nested catalogue `source_span`
offsets are preserved, not rewritten.

`ops show` selects exactly one key and joins the catalogue with the
report-backed template and operand analyses:

- **join key**: `OperationRecord.table_key == HandlerTemplate.table_key ==
  LiftedHandler.table_key`;
- **table/source identity**: source hash, ordinal, names, counts, and
  `Analyses.source_hash` agree with the report;
- **span/row consistency**: spans are equal, each row is self-consistent, and
  its `handler_index` matches the catalogue row;
- **uniqueness**: duplicate rows are ambiguous, a missing row is unmatched, a
  disagreement is inconsistent.

| Join status | Meaning |
| --- | --- |
| `joined` | One row and every identity agree |
| `unmatched` | No row for the key |
| `ambiguous` | Multiple rows; none chosen |
| `inconsistent` | Table, source, index, or span disagrees |
| `unavailable` | The analysis failed or was not requested |

Each join records `table_identity`, `source_identity`, and `span_identity` as
`verified`, `mismatch`, or `unavailable`. On failure no handler is attached,
`join.reasons` lists disagreements, and there is no fallback to a guess.

| Fact | Owner | Example values |
| --- | --- | --- |
| Inventory / classification | Catalogue | `atomic`, `composite`, `unknown` |
| Extracted recipe | Handler templates | `executable`, `partial`, `unsupported` |
| Recipe admission | `admit_recipe` | `admitted`, `rejected` |
| Operand lifting | Operand separation | `lifted`, `partial`, `unresolved`, `not_applicable` |

Admission uses the operand-lifted recipe only when lifting is `lifted` and that
recipe is admitted; otherwise it reports the extracted-recipe decision. The
`basis` is `operand_lifted_recipe` or `extracted_recipe`; a rejection carries the
first admission error code. Rejection does not change catalogue classification.

For an inventory row that cannot run, open `ops show` before doubting the
classification: [handler templates](handler-templates.md#three-extraction-boundaries-in-the-tests)
separate a known step from an unavailable recipe, and
[operation table](operation-table.md#sparse-keys-and-declined-conversions)
explains a declined readable conversion.

The `ops show` markdown order is fixed: identity header, description, handler
definition, ordered micro-operations, recipe, operands, source, effects,
evidence, catalogue unknown fragments, recipe unknown fragments, unresolved
reasons, analyst annotation. The description is a generated aid; the definition
table is the decision surface.

`--annotation` is display text only. Markdown renders a note with a disclaimer;
JSON records `authority: "analyst_note"` and `execution_authority: false`. It
cannot change any status or execution decision.

`ops show` does **not** choose an image, PC, dispatch state, or completion
contract, and it does not project an instruction occurrence. A key plus a span
is a handler, not an occurrence; the API is covered in
[ops inspection](ops-inspection.md).

## Versioned and unversioned output

Two serialization contracts coexist and change for different reasons:

| Surface | Versioned | Stability | Primary consumer |
| --- | --- | --- | --- |
| `catalogue_json` | `shape-kernel-operation-catalogue/v1` | Byte-stable layout and handler key order | External tools and snapshots |
| `ops list` / `ops show` | No version field | Existing keys/order preserved; additions append | Humans and scripts |

Views carry `derived_from` naming the catalogue `schema_version` (as `report`),
`catalogue_version`, and FNV `source_hash` (`algorithm: "fnv1a64"`). They reuse
the JSON layout, not the schema: a display change must not alter the versioned
object, and `ops list` bytes must not change when show joins more analyses.

`ops show` JSON is additive: existing top-level keys keep order and meaning, and
`status` is first inside each analysis object. An included analysis carries
schema version, source hash, coverage, join, and handler; an unavailable one
carries `reason`; a not-included one carries only `status`. The show handler
object uses `key`/`index`; the versioned record uses
`table_key`/`handler_index`. Do not conflate them.

## Sparse keys and presentation

A valid key can be absent: handlers at 0, 2, 3 keep keys 0, 2, 3 and
`missing_sparse_entries: 1`. Keys are never compacted; position 2 opens key 3.

Readable presentation may replace a proven array with a numeric-keyed object,
but only when behavior is preserved. The
[operation-table guide](operation-table.md#sparse-keys-and-declined-conversions)
explains declines for observable length, computed writes, receiver-sensitive
handlers, and unsupported index forms. A declined presentation keeps the array
and emits a diagnostic; the catalogue fact remains inspectable. Presentation
does not renumber keys, rename handlers, or merge catalogue with dispatch.

## Release inventory observation

Handler-inventory counts depend on the supplied input and are not part of the
canonical reference set. A release run must report its own exact, partial,
unknown, and sparse counts; do not generalize a count from one input to another.
The tracked JavaScript and WebAssembly VM references establish their own
operation tables (241 and 201 keys), but those sizes are inventory facts, not
dispatch identity. A low `exact` count is expected: real handlers add a return,
access, or call, so `atomic` needs one action and no unknowns.

Do not subtract a different analysis's counts. Template `executable` and operand
`lifted` counts come from different recognizers, and a handler can move
categories when either improves.

## Runtime and performance boundary

The default pipeline has no catalogue cost. Catalogue work happens only when an
`ops` command or library caller asks for it.

1. Read and hash the input bytes, then run deobfuscation to one `PipelineReport`.
2. Parse/analyze `report.output`, count declarators to the ordinal, scan that
   table once, and visit each retained handler body once.
3. Build inventory, fingerprints, and groups, then serialize one projection.

No per-binding whole-AST loop, execution, network, engine call, or extra stage
runs. Memory scales with handler count, not capture size. `--pretty` reparses
serialized JSON for display only, preserving large numeric lexemes. Nothing is
cached, and the CLI does not read `shape report`.

## Failure and troubleshooting

Library failures are a closed set:

| Error | When |
| --- | --- |
| `Parse(message)` | Standardized output does not parse |
| `NoProvenTable` | No `proven_operation_tables[0]` |
| `TableIdentityMismatch(message)` | Ordinal absent, or scanned shape/counts differ |

The CLI maps operational failures to exit 1 and usage failures to exit 2. A
missing table is not an empty catalogue; an unknown key is not a no-op.

| Symptom | Inspect |
| --- | --- |
| `ops list` says there is no proven table | The deobfuscation report and its table proof |
| `declaration ordinal N was not found` | Report/source pair or a stale hand-off |
| `expected X elements/Y functions, found A/B` | Table shape changed, or wrong source |
| A known key is absent | Sparse hole or wrong capture; list keys, do not guess position |
| A familiar handler is `unknown` | Recognizer boundary or unproven helper binding |
| `composite` when `atomic` was expected | Extra return/property/operator/call actions |
| Different handlers share a fingerprint | Same normalized shape; inspect body and free names |
| `ops show` analysis is `unavailable` | Its `reason`; the catalogue row is still valid |
| A join is `inconsistent` | Mutated table/source/span or another report |

The rule is constant: change the producer that owns the failed fact, not the
display field that reports it.

## Tests and fixtures

| Suite | Covers |
| --- | --- |
| `tests/operation_catalogue.rs` | Rename/shuffle stability, arithmetic families, fused order, duplicate groups, opaque `unknown`, confidence, sparse keys, byte-stable JSON/views |
| `tests/ops_inspection.rs` | Four-status separation, operand rendering, additive JSON, annotation, mutated joins, occurrence/decode APIs |
| `tests/operation_object.rs`, `tests/cli.rs` | Presentation, sparse keys, idempotence, help/usage/exit codes, suffix independence |
| `tests/op_classify.rs` | Cross-capture classes, natures, scopes, handler grouping |

Byte-stability fixtures pin the versioned layout and control-character escaping
([`operation-catalogue-serde.expected.json`](../../tests/fixtures/operation-catalogue-serde.expected.json)),
plus the unversioned list/show projections
([`kernel.ops-list.expected.json`](../../tests/fixtures/kernel.ops-list.expected.json),
[`kernel.ops-show-0.expected.json`](../../tests/fixtures/kernel.ops-show-0.expected.json)).
The compact seven-handler
[`handlers.raw.js`](../../tests/fixtures/ops-inspection/handlers.raw.js) is used
throughout the inspection guide.

```sh
cargo test --test operation_catalogue --test ops_inspection \
  --test operation_object --test cli --test op_classify
```

## Reading the code in order

`table_scan.rs` first: ordinal walk, both scan modes, canonical key rules, and
the two element-count methods. Then `catalogue.rs` from types and
`Classification` through `catalogue_from_report`, inventory, fingerprints,
`classify_handler`, and the recognizer; `normalize_handler` explains the
fingerprints. Then `view/mod.rs`, `view/json.rs`, and `view/show.rs` for joins
and admission. `shape.rs` is the CLI boundary. `tests/operation_catalogue.rs`
beside `catalogue.rs` specifies classification and grouping.

## Open questions

Open questions for the next foundation are the static 234-branch mapping,
operand widths and bytecode bounds, program-image mutability, exact helper
bindings in additional captures, and the state/memory ownership contract. They
remain unknown here rather than being filled by operation names.

It also does not answer which handler a selector chose, whether same-shaped
handlers captured different free values, or how new helper names map. Each
needs its own proof.

## Related guides

- [Ops inspection](ops-inspection.md) — `ops show` contracts and occurrence/decode APIs.
- [Operation table](operation-table.md) — identity, sparse keys, presentation.
- [Handler templates](handler-templates.md) — recipes, statuses, admission.
- [JavaScript operand separation](js-operand-separation.md) — byte reads and PC effects.
- [Bounded dispatch](../tracing/javascript-vm-prefix.md) — the consuming runtime join.
- [VM flow](../reference/vm-flow.md) — end-to-end extraction and execution.
- [Operation reference](../reference/operation-reference.md) — primitives and source anchors.
- [Operations module guide](../../src/operations/README.md) and
  [view guide](../../src/operations/view/README.md) — file ownership and reading paths.
