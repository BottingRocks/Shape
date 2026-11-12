# Command-line implementation

The binary entry point [shape.rs](../shape.rs) dispatches into this folder;
these modules are separate from the public library in [src/](../src/).
[mod.rs](mod.rs) owns shared source reading, stdout and broken-pipe handling,
diagnostics, usage text, JSON formatting, and atomic output helpers.

Command families are implemented in their corresponding Rust modules:

- [deob.rs](deob.rs) runs source transformation; [report.rs](report.rs) compiles a source-free kernel artifact.
- [kernel.rs](kernel.rs) implements `kernel show`; [ops.rs](ops.rs) implements `ops list|show|classify`.
- [signals.rs](signals.rs) implements schema inspection and `signals show|inputs|contracts`.
- [naming/](naming/README.md) implements static naming and corpus comparison.
- [seeds.rs](seeds.rs) implements acquisition and inspection.
- [payload.rs](payload.rs) implements `pin`, `headers`, `payload`, `encode`, `decode`, `pack`, `unpack`, and the `gen` alias.
- [decode.rs](decode.rs) orchestrates raw-seed or artifact-based named signal decoding with typed capture/session inputs.
- [trace/](trace/README.md) implements thread, program, corpus, and cleanup-thread tracing.

From the repository root, `cargo run --release --bin shape -- --help` shows
the actual dispatch surface. Command flags belong to each parser: for example,
`kernel show <source> --format json --pretty` and
`ops show <source> --op 0 --format markdown`; trace commands support `--json`.
Read the command's help before assuming flags are shared.

Maintain exact source bytes and their identities across parser/library handoffs.
Static analysis never executes recovered JavaScript or WebAssembly.
Keep usage errors, operational failures, and successful closed pipes distinct.
CLI coverage includes [generator_cli.rs](../tests/generator_cli.rs),
[naming_threads_cli.rs](../tests/naming_threads_cli.rs), and
[naming_corpus_cli.rs](../tests/naming_corpus_cli.rs); parser unit tests also
live beside their implementations. Continue with the [trace command guide](trace/commands/README.md)
and [project overview](../README.md).

`shape show signals [SignalName] [--json]` displays the source-independent
catalogue, in alphabetical canonical-name order. Text includes explanations,
wire field tables and a captured decoded payload. JSON returns an array
for all cards or an object for one exact canonical name, with parsed
`sample_payload` values. Uncaptured cards have `sample_payload: null` as metadata absence;
completion-only cards also have `no_data: true`. Unknown names and invalid arguments exit 2. The existing
`shape signals show <program.js>` command still analyzes generator inputs.
