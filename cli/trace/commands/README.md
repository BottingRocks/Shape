# Trace command front ends

Each command owns its flags, output, and exit policy. `mod.rs` routes commands and writes the shared JSON envelope.

| File | Responsibility |
| --- | --- |
| [thread.rs](thread.rs) | Recover one function from source or an artifact; optional input snapshot |
| [program.rs](program.rs) | Assemble and clean reachable functions into one program |
| [corpus.rs](corpus.rs) | Fingerprints, baseline comparisons, and branch reports |
| [cleanup_thread.rs](cleanup_thread.rs) | Clean emitted code without tracing |

Every command supports `--json`. `--out` still saves JavaScript; stdout contains only the JSON result. Summaries remain
on stderr. Flags and existing exit codes apply to both formats. See [the contract](../../../docs/tracing/trace-json.md)
and integration tests.
