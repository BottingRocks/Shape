# Generator answers and source references

These inputs protect serialization, framing, encryption, and seed pinning.
[known-answers.json](known-answers.json) supplies byte-codec cases to
[generator_codec.rs](../../../tests/generator_codec.rs) and
[generator_cli.rs](../../../tests/generator_cli.rs).
[report-known-answers.json](report-known-answers.json) covers reports, special
values, channel lengths, keyed names, and payload tails for
[generator_report.rs](../../../tests/generator_report.rs) and
[generator_header_report.rs](../../../tests/generator_header_report.rs).
[superpack-known-answers.json](superpack-known-answers.json) supplies tagged
value/hex answers to [generator_superpack.rs](../../../tests/generator_superpack.rs).
Its `decodable` flag distinguishes truncated metadata controls from round trips.

[generator_pin.rs](../../../tests/generator_pin.rs) consumes the seed-pinned
program, initializer, layout JSON, capture-header JSON, and capture-report JSON.
Keep their source identities and cross-file relationships together; they are
fixed inputs, not newly acquired session data.

The `.cjs` utilities are explicit runnable development references, distinct from
raw kernel fixtures. [superpack-extract.cjs](superpack-extract.cjs) requires two
positional source paths: `node tests/fixtures/generator/superpack-extract.cjs <source> <alternate-source>`.
It verifies both complete UTF-8 source SHA-256 identities against hardcoded
constants before writing either extraction, then copies fixed line ranges with
lexical/export glue and records reference digests in provenance JSON.
Do not run extraction or regenerate its outputs for documentation changes.

[superpack-vectors.cjs](superpack-vectors.cjs) compares independent baseline and
alternate reference encoders byte-for-byte. `--check` compares the generated
text with the checked-in answers; without it the script writes the answer file.
The Rust oracle test also verifies reference bytes against each provenance digest.
[report-vectors.cjs](report-vectors.cjs) verifies the factory, cipher, and reference
file hashes from [report-provenance.json](report-provenance.json), pins entropy,
and supports the same `--check` versus write distinction for report answers.
The payload factory/cipher sources are dependencies of that controlled harness,
not instructions to run a recovered browser program.

Preserve golden bytes, source ranges, numeric encodings, sentinel identities,
and the independence of the two encoders. A software fallback retained in a
reference does not establish compatibility for that path. Use release builds
for expensive seed generation. Read the linked test assertions and
[fixture overview](../README.md) before changing any answer.
