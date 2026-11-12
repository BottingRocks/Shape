# Explicit source decoding checkpoint

Use the [SDK guide](../../sdk/rust/README.md) for public decode integrations and
the [decode module guide](../../src/signals/decode/README.md) for source-format
compiler and typed writer contracts.

The default CLI accepts a traced program, parsed session and typed capture
headers, or a saved artifact for later source-free decoding:

```sh
shape decode --seed program.js --input-kind program \
  --session session.json --headers headers.json --float-byte-order little \
  --save-artifact artifact.json --out decoded.json --schema-out schema.json
shape decode --artifact artifact.json --headers headers.json --out decoded-again.json
cmp decoded.json decoded-again.json
```

Use the CLI `--direct` option or SDK `artifact.compile` with `"direct": true`
for explicit non-lifecycle source decoding. This path is opt-in:

```sh
shape decode --direct --seed program.js \
  --session session.json --headers headers.json --float-byte-order little \
  --verify --save-artifact direct-artifact.json --out decoded-direct.json
shape decode --artifact direct-artifact.json --headers headers.json \
  --verify --out decoded-direct-again.json
```

`--direct` defaults `--seed` to a traced program, so no `--input-kind` is
needed for `program.js`. For a raw seed, add `--input-kind raw` to trace first.
Direct compilation defaults the float byte order to little, as the SDK does;
use `--float-byte-order big` for a capture produced that way.
The default without `--direct` remains raw input and strict prove-or-refuse
compilation. A parsed `--session` is sufficient; the source, session and
capture must still match. No additional initializer input is required.

Direct compilation retains supported strict recipes and uses the reusable
source-format compiler for unsupported or initializer-dependent slots. This
includes packed counter choices whose source count is not statically proven
(the MatchMedia grid loop), so count-dependent arms decode for any count rather
than being hardcoded. It also includes the LocalStorage root projection fallback;
MatchMedia uses the normal projector. It bypasses lifecycle and strict algorithm
certification, so a successful decode is not strict consumer support certification.
Unknown source constructs can still be refused. Signals absent from the capture
remain missing; direct mode does not invent observations.

Artifacts persist direct provenance and load in that mode automatically through
the CLI or any SDK generator. `--direct` requires `--seed` and is rejected with
`--artifact` or `--schema-only`; loading a strict artifact cannot upgrade it to
direct. `--verify` checks exact signal and header re-encoding; saving an artifact
or exporting a schema after decoding always requires the same check.

The SDK equivalent is `artifact.compile` with `"direct": true` (default false).
If present this field must be a JSON boolean, including on raw-input requests;
null, strings and numbers fail before source analysis. See the
[shared SDK guide](../../sdk/README.md#workflow) for language options.

Use caller-owned input files. Static source analysis never executes JavaScript.
Unresolved certification gates fail rather than claiming strict consumer support.
The decode_workflow integration suite uses
[synthetic regression inputs](../../tests/fixtures/sdk/decode/README.md) and
[independent golden bytes](../../tests/fixtures/signal-wire-roundtrip/README.md):

```sh
cargo test --release --test decode_workflow
```

Private programs, initializer material and generated artifacts stay local.
