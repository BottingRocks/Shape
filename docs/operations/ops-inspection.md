# Ops inspection contract

`shape ops show` inspects one proven operation handler from one
`PipelineReport`. It joins the versioned catalogue, the handler-template
analysis, and the JavaScript operand-separation analysis, and reports a
concrete instruction-occurrence projection through the library API.

This page owns the current Markdown and additive JSON contract for `ops show`
and the implemented occurrence/decode API. Command-line occurrence rendering
is outside the current command surface.

## Commands

For a real tracked capture, run this from the repository root after
[building `shape`](../../README.md#build-once):

```sh
./target/release/shape ops show tests/fixtures/kernels/javascript/kernel.raw.js \
  --op 0 --format markdown
```

The table key is local to this capture. Use `ops list` to find other keys.
The general command forms are:

```text
shape ops list <source> [--format json|markdown] [--pretty]
shape ops show <source> --op KEY [--format json|markdown] [--pretty] [--annotation TEXT]
```

`ops list` is unchanged: it still projects only the unversioned catalogue list
and does not join per-handler analyses. The versioned catalogue serializer
(`shape-kernel-operation-catalogue/v1`, `catalogue_json`) is also unchanged and
remains byte-stable.

`--annotation TEXT` is a display-only analyst note. It is serialized with
`authority: "analyst_note"` and `execution_authority: false`. It never changes
classification, recipe extraction, recipe admission, operand lifting, join
status, or any execution decision.

## Work through one inspected handler

This repository has a compact seven-handler fixture. Run:

```sh
./target/debug/shape ops show tests/fixtures/ops-inspection/handlers.raw.js \
  --op 0 --format markdown
```

The `Source` section shows a handler that reads two adjacent bytes from
`vmNumberTable`, advances `context.byteOffset` by 2, writes the resulting
value to stack position 0 and memory slot 0, then returns the slot read. The
catalogue classifies it as a composite with partial confidence because its
generic call recognizer does not prove the three memory-method calls. The
extracted template is also `partial`: its inline byte reads and PC write are
unknown to that extractor. Operand separation, however, proves one unsigned
little-endian operand at byte offset 0, width 2, with entry coordinate
`after_opcode` and PC delta 2. Admission succeeds on the **lifted** recipe,
not by pretending the original extracted recipe had no unknowns.

The JSON view of that same key makes the distinction machine-readable:
`analyses.handler_templates.status` and
`analyses.operand_separation.status` are both `included`, both joins are
`joined`, the extracted template's own `status` is `Partial`, and
`analyses.recipe_admission` is
`{"status":"admitted","basis":"operand_lifted_recipe"}`. A single
generic `supported` field would hide this difference.

Read the `Handler definition` table before the generated description if you
need an execution decision. The description summarizes evidence, while the
table names its source and status. Read `Ordered micro-operations` for the
catalogue's source order, `Recipe` for admitted step order, and `Operands`
for decoded-byte roles. Their counts need not match: one source expression can
produce several catalogue micro-ops, and lifting can replace several source
steps with one certified operand. Every span on this page refers to the
standardized pipeline output whose digest appears in `Span source`; it does
not index the raw fixture bytes.

## One report, three analyses

`shape.rs` builds `Analyses::from_report(&report)` once and threads it into
`OpsView`. `Analyses` runs the existing public analyses without merging their
scanning policies:

- `catalogue_from_report` — inventory/classification and ordered micro-ops.
- `handler_templates_from_report` — extracted recipes (`let`, typed operations,
  unsupported fragments, template status).
- `js_operand_separation_from_report` — certified operand descriptors and the
  operand-lifted recipe.

The catalogue is rendered independently, as before. The two optional analyses
are separate `AnalysisData` values: `Included`, `Unavailable { reason }`, or
absent (`not_included` for `ops list`).

## Join key and validation

The handler-definition join key is the proven table key:

```text
OperationRecord.table_key
  == HandlerTemplate.table_key
  == LiftedHandler.table_key
```

All three producers take the key from the same proven table entry, so the key
is not inferred from a name, a handler index, or an assumed equality. For each
selected catalogue row the inspection validates:

1. **Table identity** — declaration ordinal, canonical/original table name,
   element count, and handler count agree with the catalogue and with the
   single `PipelineReport` table.
2. **Source identity** — each `Analyses.source_hash` (raw-input FNV-1a-64,
   not the standardized-source SHA-256 on `HandlerIR`) equals the report's
   raw-input FNV hash. See
   [identity fields named `source_hash`](operations.md#identity-fields-named-source_hash).
3. **Span identity** — catalogue, extracted-template, and lifted-handler spans
   are identical.
4. **Row consistency** — each row is self-consistent
   (`handler_index == table_key`), and the joined `handler_index` equals the
   catalogue row's index.
5. **Key uniqueness** — duplicate rows are `ambiguous`; a missing row is
   `unmatched`; a disagreement is `inconsistent`. No case falls back to a
   guessed row.

The per-analysis JSON `join` object records `status`, the joined `table_key` and
`handler_index`, the three identity checks, and every disagreement reason.

## Four separate statuses

The inspection never collapses these facts:

| Fact | Source | Example values |
|---|---|---|
| Inventory / classification | catalogue | `atomic`, `composite`, `unknown` |
| Extracted recipe | handler templates | `executable`, `partial`, `unsupported` |
| Recipe admission | `admit_recipe` on the lifted or extracted recipe | `admitted`, `rejected` |
| Operand lifting | operand separation | `lifted`, `partial`, `unresolved`, `not_applicable` |

Admission uses the operand-lifted recipe only when lifting is `lifted` and that
recipe is admitted; otherwise it reports the extracted-recipe decision. The
basis (`operand_lifted_recipe` or `extracted_recipe`) and the first admission
error code are explicit.

## Markdown contract

Sections appear in this order:

1. `# Operation` identity header (capture SHA-256, analysis, table, handler,
   kernel identity, family/class, fingerprint, confidence, span source, legacy
   FNV hash).
2. `## Description` — a generated summary of micro-op counts, recipe status,
   admission, operand lifting, and each operand's proven role. Roles are
   derived from the lifted recipe: a `Let` temporary origin maps later
   temporary uses back to the operand position, so role text shows both the
   direct use and `via temporary`.
3. `## Handler definition` — the four statuses above, with join evidence.
4. `## Ordered micro-operations` — catalogue order, operator, operand roles,
   and span.
5. `## Recipe` — extracted steps by `order`, operation name, and span. When the
   operand-lifted recipe used for admission differs in length, its step count
   is stated.
6. `## Operands` — lifting status, entry coordinate, display-only input
   binding identity and declaration span, PC delta, join, unresolved reasons,
   and certified descriptors: position, byte offset, width, byte order,
   signedness, mask (`none` or hexadecimal), descriptor PC delta, and proven
   role. An operand with no proven use prints `role unresolved`.
7. `## Source`, `## Effects`, `## Evidence`, `## Unknown fragments` — existing
   catalogue facts.
8. `## Recipe unknown fragments` — extracted-template unsupported fragments.
9. `## Unresolved` — join failures, operand-lifting reasons prefixed
   `operand separation:`, and rejected admission prefixed `recipe admission:`.
10. `## Analyst annotation` — `None supplied.`, the supplied text as a
    display-only note, or an explicit empty-note line.

## Additive JSON contract

`ops show` keeps every existing top-level key and the existing handler object in
the same order. New keys are appended; no existing key changes meaning.

```text
{
  "capture": ...,
  "derived_from": ...,
  "table": ...,
  "handler": ...,
  "span_source": ...,
  "analyses": {
    "handler_templates": {
      "status": "included",
      "schema_version": "shape-kernel-handler-templates/v1",
      "source_hash": "<fnv1a64>",
      "coverage": { "executable": n, "partial": n, "unsupported": n },
      "join": {
        "status": "joined|unmatched|ambiguous|inconsistent|unavailable",
        "table_key": 0,
        "handler_index": 0,
        "table_identity": "verified|mismatch|unavailable",
        "source_identity": "verified|mismatch|unavailable",
        "span_identity": "verified|mismatch|unavailable",
        "reasons": []
      },
      "handler": { ...HandlerTemplate... } | null
    },
    "operand_separation": {
      "status": "included",
      "schema_version": "shape-js-operand-separation/v1",
      "source_hash": "<fnv1a64>",
      "coverage": {
        "lifted": n, "partial": n, "unresolved": n, "not_applicable": n
      },
      "join": { ... same shape ... },
      "handler": { ...LiftedHandler... } | null
    },
    "recipe_admission": {
      "status": "admitted|rejected|unavailable|not_included",
      "basis": "operand_lifted_recipe|extracted_recipe",
      "code": "template_status|unsupported_fragment|step_limit|...",
      "detail": "..."
    }
  },
  "analyst_annotation": null | {
    "text": "...",
    "authority": "analyst_note",
    "execution_authority": false
  }
}
```

`status` remains the first key of each analysis object. When an analysis is
`unavailable`, it carries `reason` and omits the included fields. When an
analysis is `not_included` (for example the legacy library path or `ops list`),
it carries only `status`.

Compatibility note: `analyses.handler_templates` and
`analyses.operand_separation` previously always carried
`{"status":"not_included"}`. Their status value domain is extended, not
renamed. Existing keys keep their relative order. The versioned catalogue
serializer and `ops list` bytes do not change.

## Occurrence projection and decode classifier

The library now exposes an instruction-occurrence projection over an existing
`DispatchExecutionRecord`; the command line does not wire it.

```rust
project_occurrence(&DispatchPreparation, &OperationCatalogue,
                   &DispatchExecutionRecord) -> OccurrenceProjection
occurrence_json(&OccurrenceProjection) -> String
occurrence_markdown(&OccurrenceProjection) -> String
classify_recorded_operands(&DispatchExecutionRecord) -> DecodeClassification
classify_decoded_operands(Option<&[u8]>, Option<usize>,
                          Option<&LiftedHandler>) -> DecodeClassification
```

The occurrence join is deliberately unlike the handler-definition join:

```text
dispatch cell selector
  -> selected handler_index
  -> joined row table_key
  -> catalogue table_key
```

Every step is validated. The projection checks the dispatch cell
(`state_before`, `opcode`) selector and committed `state_after`, the certified
operand-entry coordinate (`after_opcode`/`at_opcode`), and the handler spans
across the template, operand row, and catalogue. A broken step becomes
`unmatched`, `ambiguous`, or `inconsistent` with explicit missing context; it
is never repaired by a name or a second guess.

Decode classifications are distinct:

| Status | Meaning |
|---|---|
| `decoded` | Every certified descriptor decoded from the supplied image. |
| `missing_context` | No image, entry PC, handler, or certified descriptor set. |
| `truncated` | The image ended inside an operand; the required end is reported. |
| `unsupported` | The descriptor itself cannot be decoded (for example zero width). |
| `not_applicable` | The handler has no inline operands, or the record is the terminal completion record. |

`classify_recorded_operands` uses the operands already committed by dispatch and
never invents values for a completion record. `classify_decoded_operands` is
the prospective form: it applies the existing `bounded_decode_operands` bounded
descriptor decoder and classifies its result.

## Missing joins and command support

The [view module guide](../../src/operations/view/README.md) maps the current
join, Markdown/JSON, and occurrence implementation files.

- **CLI occurrence wiring.** Rendering an occurrence needs an explicit image
  PC/state pair, a chosen completion contract, and a completion decision. That
  runtime wiring is not part of this slice; the API and its tests are.
- **WASM instruction occurrences.** `src/kernel/wasm/mod.rs` and
  `src/transforms/deob/wasm_bindings.rs` are outside this slice. No WASM occurrence
  claim is made.
- **Prospective occurrence rendering in a command.** The decode classifier is
  implemented, but no `shape` command asks for an unrecorded PC/state pair.
- **`ops list` enrichment.** Joined facts are per-handler and stay in
  `ops show`; the list projection is intentionally unchanged.
- **Non-identity `handler_index`/`table_key` captures.** No real fixture exists.
  Constructed tests mutate rows and prove the inspection reports
  `inconsistent` rather than guessing.

These are separate missing inputs. A recorded dispatch occurrence already has
an observed PC/state and can be classified by the library projection. A
prospective occurrence needs the caller to select an image, PC/state, and
completion policy before any command can display it. A WASM occurrence also
needs the interpreter selector and handler-table relationship proved; a
numeric import name by itself is insufficient. Enriching `ops list` would
change a presentation surface, not supply either missing execution proof.

## Inputs to an `OpsView`

The caller supplies a catalogue, capture identity, span-source identity, and
optional `Analyses`. `Analyses::from_report` extracts templates and operand
separation from one report. A failed analysis is retained as
`AnalysisData::Unavailable { reason }`, so the view explains the catalogue
instead of pretending the analysis was empty.

| Field | Role |
| --- | --- |
| `catalogue` | Versioned handler facts, table, inventory |
| `capture` | Identity of the exact supplied bytes |
| `span_source` | Coordinate space for handler spans |
| `analyses` | Template and operand joins; show only |
| `annotation` | Display-only note |

`Capture` has `input_kind`, a SHA-256 `digest`, byte length, and `raw_identity`.
Use `verified` only for raw bytes; use `unavailable` when only deobfuscated
source exists (all callers here pass `verified`). `SpanSource` is the SHA-256
of `report.output`; handler spans index that output, not raw bytes. `ops list`
passes `None` for analyses and span source; `ops show` supplies both.

An `OpsView` borrows catalogue and analysis data for presentation; it is not
a prepared execution object and holds no engine, image, or dispatch state. The CLI
builds one report in [`shape.rs`](../../shape.rs) and derives everything
from it.

## Selecting a handler

Use `available_keys` and `handler_by_key`, then render with `ops_list_json` /
`ops_list_markdown` or `ops_show_json` / `ops_show_markdown`. `available_keys`
sorts and deduplicates keys; `handler_by_key` returns the first match.

Select by `table_key`, never by vector position. A sparse table has holes:
handlers at positions 0, 2, and 3 have keys 0, 2, and 3, while the vector has
positions 0, 1, and 2. Selecting position 2 would open key 3.
`missing_sparse_entries` counts the holes. `handler_index` and `table_key` stay
separate fields; a disagreeing row is self-inconsistent. See
[sparse keys](operation-table.md#sparse-keys-and-declined-conversions).

`ops list` prints keys; `ops show` requires one numeric `--op KEY`. An absent
key exits 2 and lists keys; no proven table exits 1. Both accept
`--format json|markdown` (`md` alias), default to JSON, and allow `--pretty`
only for JSON. Keys are capture-local.
