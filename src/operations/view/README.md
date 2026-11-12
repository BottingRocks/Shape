# Operation inspection views

`OpsView` presents an operation catalogue and optional recipe/operand analyses.
It validates their joins and exposes handler inspection as Markdown or JSON.
Its instruction-occurrence API also projects supplied dispatch evidence.
Rendering does not extract new facts or grant execution authority.

The [inspection contract](../../../docs/operations/ops-inspection.md) owns output fields,
section order, joins, occurrence decoding, and worked cases. The
[catalogue contract](../../../docs/operations/operations.md) owns inventory serialization.

## Entry points and inputs

Build the catalogue and `Analyses::from_report(&report)` from the same report,
then pass them to `OpsView` with capture and standardized-source metadata.
The template and operand analyses have independent success/failure results;
one failure must not suppress the other. `Analyses::new` supports explicit
injection, including tests of unavailable or mismatched data.

`ops list` uses only the catalogue. `ops show --op KEY` selects a capture-local
table key, including sparse keys; it does not index the handler vector. For a
first inspection after building the executable:

```sh
shape ops show tests/fixtures/ops-inspection/handlers.raw.js --op 0 --format markdown
```

An optional analyst annotation is display-only. Keep `execution_authority:
false` and do not feed its text into recipe or decoder decisions.

## Contracts to preserve

The join checks table, source, span, row identity, and key uniqueness. Missing,
duplicate, or inconsistent analysis rows remain explicit; no fallback guesses
which row was intended. Keep catalogue classification, extracted-template
status, operand-lift status, and recipe admission as four separate facts.

Admission uses an admitted fully lifted recipe when available, otherwise the
extracted recipe decision. The displayed basis matters: a partial extracted
recipe does not become executable merely because another analysis succeeded.

Capture SHA-256, raw-input FNV, and standardized-source SHA-256 describe
different byte bases. Labels supplied to a view are not a fresh source proof.
Source spans refer to standardized output, not raw capture bytes.

Instruction occurrences need actual dispatch records and matching preparation.
Recorded and prospective operand classifiers have different missing-context
and truncation outcomes. Completion records are not ordinary handler decodes.
The occurrence API is library-only; current `ops show` does not accept arbitrary
image/PC/state inputs to manufacture a trace.

## Files and diagnosis

| File | Responsibility |
| --- | --- |
| [`mod.rs`](mod.rs) | View inputs, analysis containers, selection, list rendering |
| [`show.rs`](show.rs) | Handler joins, admission basis, descriptions, Markdown |
| [`json.rs`](json.rs) | Additive JSON projection |
| [`occurrence.rs`](occurrence.rs) | Instruction projection and operand classifiers |

For wrong facts, inspect the producer. For correct facts on the wrong row,
inspect selection and joins. For a decode mismatch, distinguish recorded
operands from a prospective decode before inspecting widths and coordinates.
Do not fix a presentation issue by weakening identity checks.

## Checks

```sh
cargo test --test operation_catalogue --test ops_inspection
```

The seven-handler inspection fixture pins partial extraction, successful
lifting, rejected admission, sparse/missing joins, Markdown, and JSON. Preserve
catalogue-only output while extending inspection fields; test annotations as
non-authoritative and errors as visible results.
