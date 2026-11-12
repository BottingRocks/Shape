# Naming commands

[mod.rs](mod.rs) dispatches `shape naming threads|collectors|inventory|plan|corpus`.
All paths analyze source statically; none execute recovered JavaScript.

[threads.rs](threads.rs) parses focused renaming options. Both `threads` and
`collectors` accept `--map-out FILE` and paired `--rename --out FILE`;
outputs must be new files. Threads preserve readable names and can remove
proven unreferenced empty scoped stubs. Collectors rename only proven collector
bindings and skip the general naming and signal grammar passes.
Threads also support `--labels FILE`, or reference creation with
`--original TRACE --save-labels NEW.json` on the unpruned readable input.

`inventory <program.js> [--json]` reports the function index and role graph.
`plan <program.js>` always prints JSON proposals, proofs, ambiguities, and
refusals; its parser rejects `--json`.

[corpus.rs](corpus.rs) runs the naming chain and compares fingerprints.
Important options include `--out DIR`, `--json`, `--baseline FILE`, and
`--update-baseline` (which requires `--baseline`). A reviewed baseline is a
control, not an automatically refreshed expected answer. See its help and
[corpus documentation](../../docs/naming/corpus.md) for input manifests and tracing options.

[naming_threads_cli.rs](../../tests/naming_threads_cli.rs) and
[naming_corpus_cli.rs](../../tests/naming_corpus_cli.rs) exercise the command boundary.
Preserve original spans, declaration ownership, resolved reference edits, and
no-clobber behavior when changing dispatch or output handling. Read the
[library naming guide](../../src/naming/README.md), [CLI overview](../README.md),
and [trace inputs](../trace/README.md) next.
